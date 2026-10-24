// ###################
// Songs for the <audio> element, over a small HTTP server on 127.0.0.1. WebKitGTK can't stream
// media over Tauri's asset:// protocol on Linux (pictures work, audio doesn't), but plays http://.
// Safe by design: it listens only on this computer, every request needs a key made at start,
// and it serves only songs in the library, by id (never a path from the request). Range
// requests are answered, so seeking works
// ###################

use crate::db::Db;
use rusqlite::OptionalExtension;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub struct Stream {
	pub base: String,   // "http://127.0.0.1:PORT/song/"
	pub key: String,
}

pub fn start(app: &AppHandle) -> std::io::Result<Stream> {
	let listener = TcpListener::bind("127.0.0.1:0")?;
	let port = listener.local_addr()?.port();
	let key = uuid::Uuid::new_v4().simple().to_string();
	let app = app.clone();
	let secret = key.clone();
	std::thread::spawn(move || {
		for connection in listener.incoming().flatten() {
			let app = app.clone();
			let secret = secret.clone();
			std::thread::spawn(move || {
				let _ = answer(connection, &secret, &|id| file_of(&app, id));
			});
		}
	});
	Ok(Stream { base: format!("http://127.0.0.1:{port}/song/"), key })
}

fn file_of(app: &AppHandle, id: &str) -> Option<PathBuf> {
	let db = app.state::<Db>();
	let conn = db.conn.lock().ok()?;
	conn.query_row("SELECT file FROM tracks WHERE id = ?1", [id], |r| r.get::<_, String>(0)).optional().ok()?.map(PathBuf::from)
}

pub fn content_type(path: &std::path::Path) -> &'static str {
	match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
		Some("mp3") => "audio/mpeg",
		Some("m4a") | Some("aac") => "audio/mp4",
		Some("opus") | Some("ogg") | Some("oga") => "audio/ogg",
		Some("flac") => "audio/flac",
		Some("wav") => "audio/wav",
		Some("aiff") => "audio/aiff",
		Some("wma") => "audio/x-ms-wma",
		_ => "application/octet-stream",
	}
}

// "bytes=100-" / "bytes=100-199" / "bytes=-500" → (start, end) inside a file of `len` bytes
pub fn parse_range(header: &str, len: u64) -> Option<(u64, u64)> {
	let spec = header.trim().strip_prefix("bytes=")?.split(',').next()?.trim();
	let (a, b) = spec.split_once('-')?;
	if len == 0 {
		return None;
	}
	let (start, end) = if a.is_empty() {
		let n: u64 = b.parse().ok()?;
		(len.saturating_sub(n), len - 1)
	} else {
		let start: u64 = a.parse().ok()?;
		let end = if b.is_empty() { len - 1 } else { b.parse::<u64>().ok()?.min(len - 1) };
		(start, end)
	};
	(start <= end && start < len).then_some((start, end))
}

fn status(stream: &mut TcpStream, code: &str) -> std::io::Result<()> {
	write!(stream, "HTTP/1.1 {code}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

// one request: GET or HEAD /song/<id>?k=<key>, with or without Range
pub fn answer(mut stream: TcpStream, key: &str, find: &dyn Fn(&str) -> Option<PathBuf>) -> std::io::Result<()> {
	let mut reader = BufReader::new(stream.try_clone()?);
	let mut first = String::new();
	reader.read_line(&mut first)?;
	let mut range = None;
	loop {
		let mut line = String::new();
		if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
			break;
		}
		if let Some((name, value)) = line.split_once(':') {
			if name.trim().eq_ignore_ascii_case("range") {
				range = Some(value.trim().to_string());
			}
		}
	}
	let mut parts = first.split_whitespace();
	let method = parts.next().unwrap_or("");
	let target = parts.next().unwrap_or("");
	if method != "GET" && method != "HEAD" {
		return status(&mut stream, "405 Method Not Allowed");
	}
	let (path, query) = target.split_once('?').unwrap_or((target, ""));
	if !query.split('&').any(|kv| kv == format!("k={key}")) {
		return status(&mut stream, "403 Forbidden");
	}
	let Some(id) = path.strip_prefix("/song/").filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) else {
		return status(&mut stream, "404 Not Found");
	};
	let Some(file) = find(id).filter(|f| f.is_file()) else {
		return status(&mut stream, "404 Not Found");
	};
	let mut handle = std::fs::File::open(&file)?;
	let len = handle.metadata()?.len();
	let kind = content_type(&file);
	let (start, end, head) = match range.as_deref() {
		Some(r) => match parse_range(r, len) {
			Some((s, e)) => (s, e, format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {s}-{e}/{len}\r\n")),
			None => {
				return write!(stream, "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{len}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
			}
		},
		None => (0, len.saturating_sub(1), "HTTP/1.1 200 OK\r\n".to_string()),
	};
	let count = if len == 0 { 0 } else { end - start + 1 };
	write!(stream, "{head}Content-Type: {kind}\r\nContent-Length: {count}\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n")?;
	if method == "GET" && count > 0 {
		handle.seek(SeekFrom::Start(start))?;
		std::io::copy(&mut handle.take(count), &mut stream)?;
	}
	stream.flush()
}

// ###################
// Tests: ranges, and a real request over a socket (key, unknown id, a range)
// ###################
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn ranges() {
		assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
		assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
		assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
		assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
		assert_eq!(parse_range("bytes=100-", 100), None);
		assert_eq!(parse_range("bytes=20-10", 100), None);
		assert_eq!(parse_range("items=0-1", 100), None);
	}

	#[test]
	fn serves_songs_by_id_with_the_key() {
		let dir = std::env::temp_dir().join(format!("moonwave-stream-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let song = dir.join("song كايروكي.m4a");
		std::fs::write(&song, (0..=255u8).collect::<Vec<_>>()).unwrap();
		let listener = TcpListener::bind("127.0.0.1:0").unwrap();
		let port = listener.local_addr().unwrap().port();
		let known = song.clone();
		std::thread::spawn(move || {
			for c in listener.incoming().flatten() {
				let known = known.clone();
				let _ = answer(c, "secret", &move |id| (id == "abc-1").then(|| known.clone()));
			}
		});
		let ask = |request: &str| {
			let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
			s.write_all(request.as_bytes()).unwrap();
			let mut out = Vec::new();
			s.read_to_end(&mut out).unwrap();
			out
		};
		let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
		let whole = ask("GET /song/abc-1?k=secret HTTP/1.1\r\nHost: x\r\n\r\n");
		assert!(text(&whole).starts_with("HTTP/1.1 200 OK"));
		assert!(text(&whole).contains("Content-Type: audio/mp4") && text(&whole).contains("Content-Length: 256"));
		let part = ask("GET /song/abc-1?k=secret HTTP/1.1\r\nRange: bytes=10-13\r\n\r\n");
		assert!(text(&part).contains("206 Partial Content") && text(&part).contains("Content-Range: bytes 10-13/256"));
		assert_eq!(&part[part.len() - 4..], &[10, 11, 12, 13]);
		assert!(text(&ask("GET /song/abc-1?k=wrong HTTP/1.1\r\n\r\n")).starts_with("HTTP/1.1 403"));
		assert!(text(&ask("GET /song/other?k=secret HTTP/1.1\r\n\r\n")).starts_with("HTTP/1.1 404"));
		assert!(text(&ask("GET /song/../../etc/passwd?k=secret HTTP/1.1\r\n\r\n")).starts_with("HTTP/1.1 404"));
		assert!(text(&ask("POST /song/abc-1?k=secret HTTP/1.1\r\n\r\n")).starts_with("HTTP/1.1 405"));
		let head = ask("HEAD /song/abc-1?k=secret HTTP/1.1\r\n\r\n");
		assert!(text(&head).ends_with("\r\n\r\n"), "no body for HEAD");
		std::fs::remove_dir_all(&dir).ok();
	}

	// GStreamer (what WebKitGTK plays through) streaming a real song from this server:
	// MOONWAVE_TEST_SONG=/path/to/song.m4a cargo test gstreamer_streams -- --ignored --nocapture
	#[test]
	#[ignore]
	fn gstreamer_streams() {
		let song = PathBuf::from(std::env::var("MOONWAVE_TEST_SONG").expect("MOONWAVE_TEST_SONG"));
		let listener = TcpListener::bind("127.0.0.1:0").unwrap();
		let port = listener.local_addr().unwrap().port();
		let seen = std::sync::Arc::new(std::sync::Mutex::new(0usize));
		let count = seen.clone();
		std::thread::spawn(move || {
			for c in listener.incoming().flatten() {
				*count.lock().unwrap() += 1;
				let song = song.clone();
				std::thread::spawn(move || {
					let _ = answer(c, "k1", &move |_| Some(song.clone()));
				});
			}
		});
		let uri = format!("uri=http://127.0.0.1:{port}/song/abc?k=k1");
		let out = std::process::Command::new("gst-launch-1.0").args(["-q", "uridecodebin", &uri, "!", "audioconvert", "!", "fakesink", "sync=false"]).output().unwrap();
		println!("requests: {} | {}", seen.lock().unwrap(), String::from_utf8_lossy(&out.stderr));
		assert!(out.status.success(), "GStreamer couldn't stream it");
	}
}
