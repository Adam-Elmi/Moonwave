// ###################
// Downloading a video's audio (personal use) with the bundled yt-dlp and ffmpeg.
// A link is checked first (single YouTube videos only), then looked up (title, artist, length,
// thumbnail) so you can check it and change the names. The download runs yt-dlp with progress
// lines Moonwave reads; then Rust writes the tags and the cover (lofty) and moves the file into the
// folder you chose (commands.rs adds it to the library). yt-dlp goes out of date when YouTube
// changes: Update fetches the newest one into the data folder, which is used from then on
// ###################

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

// ###################
// The tools: the updated yt-dlp (data folder) or the bundled one (next to Moonwave), else PATH.
// On Windows they're .exe files, and they run without a console window
// ###################
pub struct Tools {
	pub data: PathBuf,   // the app's data folder
}

// a tool's file name: "ffmpeg", or "ffmpeg.exe" on Windows
pub fn exe(name: &str) -> String {
	format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

// no console window flashing up for each tool on Windows
pub fn quiet(cmd: &mut Command) -> &mut Command {
	#[cfg(windows)]
	{
		use std::os::windows::process::CommandExt;
		cmd.creation_flags(0x0800_0000);   // CREATE_NO_WINDOW
	}
	cmd
}

fn beside_app(name: &str) -> Option<PathBuf> {
	let path = std::env::current_exe().ok()?.parent()?.join(exe(name));
	path.is_file().then_some(path)
}

fn on_path(name: &str) -> Option<PathBuf> {
	std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|p| p.join(exe(name))).find(|p| p.is_file()))
}

impl Tools {
	pub fn updated(&self) -> PathBuf {
		self.data.join("tools").join(exe("yt-dlp"))
	}
	pub fn yt_dlp(&self) -> Option<PathBuf> {
		let updated = self.updated();
		if updated.is_file() {
			return Some(updated);
		}
		beside_app("yt-dlp").or_else(|| on_path("yt-dlp"))
	}
	// the folder with ffmpeg and ffprobe (yt-dlp needs both)
	pub fn ffmpeg_dir(&self) -> Option<PathBuf> {
		beside_app("ffmpeg").or_else(|| on_path("ffmpeg")).and_then(|p| p.parent().map(Path::to_path_buf))
	}
	// yt-dlp needs a JavaScript runtime for YouTube: the bundled deno (Windows), else deno, node
	// or bun from the system
	pub fn js_runtime(&self) -> Option<String> {
		let found = |name: &str, path: PathBuf| format!("{name}:{}", path.display());
		beside_app("deno").map(|p| found("deno", p)).or_else(|| ["deno", "node", "bun"].iter().find_map(|name| on_path(name).map(|p| found(name, p))))
	}
	fn command(&self) -> Result<Command, String> {
		let yt = self.yt_dlp().ok_or("The downloader (yt-dlp) is missing. Use Update downloader to fetch it.")?;
		let mut cmd = Command::new(yt);
		quiet(&mut cmd);
		cmd.args(["--no-playlist", "--no-warnings", "--no-color", "--ignore-config"]);
		if let Some(dir) = self.ffmpeg_dir() {
			cmd.arg("--ffmpeg-location").arg(dir);
		}
		if let Some(js) = self.js_runtime() {
			cmd.args(["--js-runtimes", &js]);
		}
		Ok(cmd)
	}
}

#[derive(Serialize, Debug)]
pub struct Info {
	pub version: String,
	pub updated: bool,       // the data folder's copy is in use
	pub ffmpeg: bool,
	pub js_runtime: Option<String>,
	pub folder: String,      // where downloads go
}

pub fn info(tools: &Tools, folder: &Path) -> Info {
	let version = tools
		.yt_dlp()
		.and_then(|yt| quiet(&mut Command::new(yt)).arg("--version").output().ok())
		.filter(|o| o.status.success())
		.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
		.unwrap_or_default();
	Info { version, updated: tools.updated().is_file(), ffmpeg: tools.ffmpeg_dir().is_some(), js_runtime: tools.js_runtime().map(|j| j.split(':').next().unwrap_or("").to_string()), folder: folder.display().to_string() }
}

// ###################
// Links: a single YouTube video → its watch URL; anything else → why not
// ###################
pub fn check_url(input: &str) -> Result<String, String> {
	let not_video = || "That isn’t a YouTube video link. Copy the link of one video (it has watch?v= or youtu.be/).".to_string();
	let text = input.trim();
	let rest = text.strip_prefix("https://").or_else(|| text.strip_prefix("http://")).unwrap_or(text);
	let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
	let host = host.to_ascii_lowercase();
	let host = host.strip_prefix("www.").or_else(|| host.strip_prefix("m.")).unwrap_or(&host);
	let (route, query) = path.split_once('?').unwrap_or((path, ""));
	let param = |name: &str| query.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v.to_string()));
	let id = match host {
		"youtu.be" => Some(route.split('/').next().unwrap_or("").to_string()),
		"youtube.com" | "music.youtube.com" => {
			if route == "watch" {
				param("v")
			} else if let Some(id) = route.strip_prefix("shorts/").or_else(|| route.strip_prefix("live/")).or_else(|| route.strip_prefix("embed/")) {
				Some(id.split('/').next().unwrap_or("").to_string())
			} else if route == "playlist" {
				return Err("That’s a playlist. Moonwave downloads one video at a time: open a video and copy its link.".into());
			} else {
				None
			}
		}
		_ => None,
	};
	let id = id.ok_or_else(not_video)?;
	if id.len() != 11 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
		return Err(not_video());
	}
	Ok(format!("https://www.youtube.com/watch?v={id}"))
}

// ###################
// Looking a video up
// ###################
#[derive(Serialize, Debug, PartialEq)]
pub struct Preview {
	pub url: String,
	pub title: String,
	pub artist: String,
	pub album: String,
	pub channel: String,
	pub duration_ms: Option<i64>,
	pub thumbnail: Option<String>,
}

// YouTube Music gives track / artist / album; other videos only a title and a channel
pub fn preview_from(json: &serde_json::Value, url: &str) -> Preview {
	let text = |key: &str| json[key].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
	let channel = text("channel").or_else(|| text("uploader")).unwrap_or_default();
	let channel = channel.strip_suffix(" - Topic").unwrap_or(&channel).to_string();
	let artist = json["artists"].as_array().and_then(|a| a.first()).and_then(|a| a.as_str()).map(str::to_string).or_else(|| text("artist")).or_else(|| text("creator")).unwrap_or_else(|| channel.clone());
	Preview {
		url: url.to_string(),
		title: text("track").or_else(|| text("title")).unwrap_or_else(|| "Untitled".into()),
		artist,
		album: text("album").unwrap_or_default(),
		channel,
		duration_ms: json["duration"].as_f64().map(|d| (d * 1000.0) as i64),
		thumbnail: text("thumbnail"),
	}
}

// the last "ERROR: …" line yt-dlp printed, without its prefixes:
// "ERROR: [youtube] BaW_jenozKc: Video unavailable" → "Video unavailable"
fn error_from(stderr: &str) -> String {
	let Some(line) = stderr.lines().rev().find_map(|l| l.strip_prefix("ERROR: ")) else {
		return "The downloader stopped without saying why. Try Update downloader.".into();
	};
	let line = line.split_once("] ").map(|(_, rest)| rest).unwrap_or(line);
	match line.split_once(": ") {
		Some((id, rest)) if id.len() == 11 && !id.contains(' ') => rest.to_string(),
		_ => line.to_string(),
	}
}

pub fn preview(tools: &Tools, url: &str) -> Result<Preview, String> {
	let url = check_url(url)?;
	let out = tools.command()?.args(["--dump-single-json", "--skip-download"]).arg(&url).output().map_err(|e| format!("Couldn’t start the downloader ({e})."))?;
	if !out.status.success() {
		return Err(error_from(&String::from_utf8_lossy(&out.stderr)));
	}
	let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("The downloader’s answer couldn’t be read ({e})."))?;
	Ok(preview_from(&json, &url))
}

// ###################
// Downloading
// ###################
#[derive(Deserialize, Debug, Clone)]
pub struct Request {
	pub url: String,
	pub format: String,   // m4a / opus / mp3
	pub title: String,
	pub artist: String,
	pub album: String,
	pub thumbnail: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Progress {
	pub stage: String,    // starting / downloading / converting / tagging
	pub percent: f32,
	pub speed: String,
	pub eta: String,
}

// the progress template's lines: "MW|  45.3%|1.2MiB/s|00:12"
pub fn parse_progress(line: &str) -> Option<Progress> {
	let mut parts = line.strip_prefix("MW|")?.split('|');
	let percent = parts.next()?.trim().trim_end_matches('%').trim().parse::<f32>().ok()?;
	let clean = |s: Option<&str>| s.map(str::trim).filter(|s| !s.is_empty() && *s != "NA" && *s != "Unknown").unwrap_or("").to_string();
	Some(Progress { stage: "downloading".into(), percent, speed: clean(parts.next()), eta: clean(parts.next()) })
}

// "Ayan Noor - Harbour Lights" as a file name: no slashes or other characters files can't have
pub fn file_name(artist: &str, title: &str, ext: &str) -> String {
	let base = if artist.trim().is_empty() { title.trim().to_string() } else { format!("{} - {}", artist.trim(), title.trim()) };
	let safe: String = base.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { ' ' } else { c }).collect();
	let safe = safe.split_whitespace().collect::<Vec<_>>().join(" ");
	let safe: String = safe.trim_matches('.').chars().take(150).collect();
	format!("{}.{ext}", if safe.is_empty() { "Download".to_string() } else { safe })
}

// a free path in the folder: "Name.m4a", else "Name (2).m4a", …
pub fn free_path(folder: &Path, name: &str) -> PathBuf {
	let first = folder.join(name);
	if !first.exists() {
		return first;
	}
	let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
	(2..).map(|n| folder.join(format!("{stem} ({n}).{ext}"))).find(|p| !p.exists()).unwrap()
}

// the running download, so Cancel can stop it
#[derive(Default)]
pub struct Running {
	pub child: Mutex<Option<Child>>,
	pub cancelled: Mutex<bool>,
}

pub fn cancel(running: &Running) {
	*running.cancelled.lock().unwrap() = true;
	if let Some(child) = running.child.lock().unwrap().as_mut() {
		let _ = child.kill();
	}
}

// downloads into `work`, then tags and moves the file into `folder`; returns the final path
pub fn download(tools: &Tools, running: &Running, req: &Request, work: &Path, folder: &Path, mut on_progress: impl FnMut(Progress)) -> Result<PathBuf, String> {
	let url = check_url(&req.url)?;
	let (format, audio) = match req.format.as_str() {
		"opus" => ("bestaudio[acodec=opus]/bestaudio", "opus"),
		"mp3" => ("bestaudio", "mp3"),
		_ => ("bestaudio[ext=m4a]/bestaudio", "m4a"),
	};
	if tools.ffmpeg_dir().is_none() {
		return Err("ffmpeg is missing, so the audio can’t be saved. Reinstall Moonwave.".into());
	}
	std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
	std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
	*running.cancelled.lock().unwrap() = false;
	on_progress(Progress { stage: "starting".into(), percent: 0.0, speed: String::new(), eta: String::new() });

	let mut cmd = tools.command()?;
	// --print makes yt-dlp quiet: --progress keeps the progress lines, and it says when converting starts
	cmd.args(["-f", format, "-x", "--audio-format", audio, "--audio-quality", "0", "--newline", "--progress", "--no-mtime", "--no-part"])
		.args(["--progress-template", "download:MW|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s"])
		.args(["--print", "post_process:MWSTAGE|converting", "--print", "after_move:MWFILE|%(filepath)s"])
		.arg("-o")
		.arg(work.join("%(id)s.%(ext)s"))
		.arg(&url)
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	let mut child = cmd.spawn().map_err(|e| format!("Couldn’t start the downloader ({e})."))?;
	let stdout = child.stdout.take().unwrap();
	let mut stderr = child.stderr.take().unwrap();
	*running.child.lock().unwrap() = Some(child);
	// stderr on its own thread, so a full pipe never blocks yt-dlp
	let errors = std::thread::spawn(move || {
		let mut text = String::new();
		let _ = stderr.read_to_string(&mut text);
		text
	});
	let mut file = None;
	for line in BufReader::new(stdout).lines().map_while(Result::ok) {
		if let Some(p) = parse_progress(&line) {
			on_progress(p);
		} else if let Some(path) = line.strip_prefix("MWFILE|") {
			file = Some(PathBuf::from(path.trim()));
		} else if line.starts_with("MWSTAGE|converting") {
			on_progress(Progress { stage: "converting".into(), percent: 100.0, speed: String::new(), eta: String::new() });
		}
	}
	let status = running.child.lock().unwrap().take().map(|mut c| c.wait());
	let stderr = errors.join().unwrap_or_default();
	if *running.cancelled.lock().unwrap() {
		return Err("Cancelled.".into());
	}
	if !matches!(status, Some(Ok(s)) if s.success()) {
		return Err(error_from(&stderr));
	}
	let file = file.filter(|f| f.is_file()).ok_or("The downloader finished but didn’t save a file.")?;

	on_progress(Progress { stage: "tagging".into(), percent: 100.0, speed: String::new(), eta: String::new() });
	let cover = req.thumbnail.as_deref().and_then(|u| thumbnail_jpeg(u).ok());
	tag(&file, &req.title, &req.artist, &req.album, cover)?;
	let ext = file.extension().and_then(|e| e.to_str()).unwrap_or(audio).to_string();
	let target = free_path(folder, &file_name(&req.artist, &req.title, &ext));
	if std::fs::rename(&file, &target).is_err() {
		std::fs::copy(&file, &target).map_err(|e| format!("Couldn’t save the file ({e})."))?;
		let _ = std::fs::remove_file(&file);
	}
	Ok(target)
}

// the thumbnail as a square JPEG (YouTube's are 16:9 with bars; the middle square is the cover)
fn thumbnail_jpeg(url: &str) -> Result<Vec<u8>, String> {
	let mut body = ureq::get(url).call().map_err(|e| e.to_string())?.into_body();
	let bytes = body.with_config().limit(10 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())?;
	let picture = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
	let side = picture.width().min(picture.height());
	let square = picture.crop_imm((picture.width() - side) / 2, (picture.height() - side) / 2, side, side).resize(600, 600, image::imageops::FilterType::Lanczos3);
	let mut out = std::io::Cursor::new(Vec::new());
	square.to_rgb8().write_to(&mut out, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
	Ok(out.into_inner())
}

// title, artist, album and the cover, written into the file (M4A, Opus, MP3, …)
pub fn tag(file: &Path, title: &str, artist: &str, album: &str, cover: Option<Vec<u8>>) -> Result<(), String> {
	use lofty::config::WriteOptions;
	use lofty::file::{AudioFile, TaggedFileExt};
	use lofty::picture::{MimeType, Picture, PictureType};
	use lofty::tag::{Accessor, Tag};
	let mut tagged = lofty::read_from_path(file).map_err(|e| format!("Couldn’t read the downloaded file ({e})."))?;
	let kind = tagged.primary_tag_type();
	if tagged.primary_tag().is_none() {
		tagged.insert_tag(Tag::new(kind));
	}
	let tag = tagged.primary_tag_mut().unwrap();
	tag.set_title(title.trim().to_string());
	if !artist.trim().is_empty() {
		tag.set_artist(artist.trim().to_string());
	}
	if !album.trim().is_empty() {
		tag.set_album(album.trim().to_string());
	}
	if let Some(jpeg) = cover {
		tag.remove_picture_type(PictureType::CoverFront);
		tag.push_picture(Picture::unchecked(jpeg).pic_type(PictureType::CoverFront).mime_type(MimeType::Jpeg).build());
	}
	tagged.save_to_path(file, WriteOptions::default()).map_err(|e| format!("Couldn’t write the song’s details ({e})."))
}

// ###################
// How well YouTube can be reached: two tiny requests to its "204" address; the second (on the
// connection the first opened) is the round trip, in ms. No answer within 5 s → offline (an error)
// ###################
pub fn ping() -> Result<u64, String> {
	let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(std::time::Duration::from_secs(5))).build().into();
	let url = "https://www.youtube.com/generate_204";
	agent.get(url).call().map_err(|e| format!("YouTube can’t be reached ({e})."))?;
	let start = std::time::Instant::now();
	agent.get(url).call().map_err(|e| format!("YouTube can’t be reached ({e})."))?;
	Ok(start.elapsed().as_millis() as u64)
}

// ###################
// Update downloader: the newest yt-dlp (the Linux or the Windows build), checked against its
// release's checksums
// ###################
pub fn update(tools: &Tools) -> Result<String, String> {
	use sha2::{Digest, Sha256};
	let base = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
	let fetch = |name: &str, limit: u64| -> Result<Vec<u8>, String> {
		let mut body = ureq::get(&format!("{base}/{name}")).call().map_err(|e| format!("Couldn’t reach GitHub ({e})."))?.into_body();
		body.with_config().limit(limit).read_to_vec().map_err(|e| e.to_string())
	};
	let asset = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp_linux" };
	let sums = String::from_utf8(fetch("SHA2-256SUMS", 1024 * 1024)?).map_err(|e| e.to_string())?;
	let want = sums.lines().find_map(|l| l.strip_suffix(&format!(" {asset}")).map(|h| h.trim().to_lowercase())).ok_or(format!("The release has no checksum for {asset}."))?;
	let binary = fetch(asset, 200 * 1024 * 1024)?;
	let got: String = Sha256::digest(&binary).iter().map(|b| format!("{b:02x}")).collect();
	if got != want {
		return Err("The download didn’t match its checksum, so it wasn’t used.".into());
	}
	let target = tools.updated();
	std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
	let temp = target.with_extension("new");
	std::fs::write(&temp, &binary).map_err(|e| e.to_string())?;
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
	}
	std::fs::rename(&temp, &target).map_err(|e| e.to_string())?;
	let out = quiet(&mut Command::new(&target)).arg("--version").output().map_err(|e| e.to_string())?;
	Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// ###################
// Tests (a real download runs only with: cargo test -- --ignored --nocapture real_download)
// ###################
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn links() {
		let ok = |s: &str| check_url(s).unwrap();
		assert_eq!(ok("https://www.youtube.com/watch?v=BaW_jenozKc"), "https://www.youtube.com/watch?v=BaW_jenozKc");
		assert_eq!(ok("youtube.com/watch?v=BaW_jenozKc&list=PL123&t=40s"), "https://www.youtube.com/watch?v=BaW_jenozKc", "the playlist part is dropped");
		assert_eq!(ok("https://youtu.be/BaW_jenozKc?si=abc"), "https://www.youtube.com/watch?v=BaW_jenozKc");
		assert_eq!(ok("https://m.youtube.com/shorts/BaW_jenozKc"), "https://www.youtube.com/watch?v=BaW_jenozKc");
		assert_eq!(ok("https://music.youtube.com/watch?v=BaW_jenozKc&feature=share"), "https://www.youtube.com/watch?v=BaW_jenozKc");
		assert!(check_url("https://www.youtube.com/playlist?list=PL123").unwrap_err().contains("playlist"));
		assert!(check_url("https://vimeo.com/123").is_err());
		assert!(check_url("https://www.youtube.com/watch?v=short").is_err());
		assert!(check_url("https://www.youtube.com/@channel").is_err());
		assert!(check_url("").is_err());
	}

	#[test]
	fn previews_prefer_music_details() {
		let music = serde_json::json!({ "title": "Harbour Lights (Official Audio)", "track": "Harbour Lights", "artists": ["Ayan Noor", "Guest"], "album": "Coastline", "channel": "Ayan Noor - Topic", "duration": 215.5, "thumbnail": "https://i.ytimg.com/x.jpg" });
		let p = preview_from(&music, "u");
		assert_eq!((p.title.as_str(), p.artist.as_str(), p.album.as_str(), p.channel.as_str()), ("Harbour Lights", "Ayan Noor", "Coastline", "Ayan Noor"));
		assert_eq!(p.duration_ms, Some(215_500));
		let plain = serde_json::json!({ "title": "My song", "uploader": "Some Channel" });
		let p = preview_from(&plain, "u");
		assert_eq!((p.title.as_str(), p.artist.as_str(), p.album.as_str(), p.duration_ms), ("My song", "Some Channel", "", None));
	}

	#[test]
	fn progress_lines() {
		assert_eq!(parse_progress("MW|  45.3%|1.20MiB/s|00:12"), Some(Progress { stage: "downloading".into(), percent: 45.3, speed: "1.20MiB/s".into(), eta: "00:12".into() }));
		assert_eq!(parse_progress("MW|100.0%|NA|NA").map(|p| (p.percent, p.speed, p.eta)), Some((100.0, String::new(), String::new())));
		assert_eq!(parse_progress("[download] Destination: x"), None);
	}

	#[test]
	fn errors_without_prefixes() {
		assert_eq!(error_from("WARNING: x\nERROR: [youtube] BaW_jenozKc: Video unavailable"), "Video unavailable");
		assert_eq!(error_from("ERROR: Unable to download webpage: timed out"), "Unable to download webpage: timed out");
		assert!(error_from("").contains("Update downloader"));
	}

	#[test]
	fn file_names_and_free_paths() {
		assert_eq!(file_name("Ayan Noor", "Harbour Lights", "m4a"), "Ayan Noor - Harbour Lights.m4a");
		assert_eq!(file_name("", "A/B: C?", "mp3"), "A B C.mp3");
		assert_eq!(file_name(" ", "...", "opus"), "Download.opus");
		assert_eq!(file_name("x", &"y".repeat(300), "m4a").chars().count(), 150 + 4);
		let dir = std::env::temp_dir().join(format!("moonwave-free-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		std::fs::write(dir.join("Song.m4a"), "").unwrap();
		std::fs::write(dir.join("Song (2).m4a"), "").unwrap();
		assert_eq!(free_path(&dir, "Song.m4a"), dir.join("Song (3).m4a"));
		assert_eq!(free_path(&dir, "Other.m4a"), dir.join("Other.m4a"));
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn tags_are_written() {
		let dir = std::env::temp_dir().join(format!("moonwave-tag-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let file = dir.join("t.wav");
		// one second of silence
		let mut b = Vec::new();
		b.extend(b"RIFF");
		b.extend((36u32 + 16000).to_le_bytes());
		b.extend(b"WAVEfmt ");
		b.extend(16u32.to_le_bytes());
		b.extend(1u16.to_le_bytes());
		b.extend(1u16.to_le_bytes());
		b.extend(8000u32.to_le_bytes());
		b.extend(16000u32.to_le_bytes());
		b.extend(2u16.to_le_bytes());
		b.extend(16u16.to_le_bytes());
		b.extend(b"data");
		b.extend(16000u32.to_le_bytes());
		b.extend(vec![0u8; 16000]);
		std::fs::write(&file, b).unwrap();
		let mut jpeg = std::io::Cursor::new(Vec::new());
		image::RgbImage::from_pixel(32, 32, image::Rgb([200, 40, 90])).write_to(&mut jpeg, image::ImageFormat::Jpeg).unwrap();
		tag(&file, "Harbour Lights", "Ayan Noor", "Coastline", Some(jpeg.into_inner())).unwrap();
		let info = crate::library::read_track(&file, &dir.join("covers")).unwrap();
		assert_eq!((info.title.as_str(), info.artist.as_str(), info.album.as_str()), ("Harbour Lights", "Ayan Noor", "Coastline"));
		assert!(info.cover.is_some(), "the cover went in");
		std::fs::remove_dir_all(&dir).ok();
	}

	// the real round trip to YouTube: cargo test real_ping -- --ignored --nocapture
	#[test]
	#[ignore]
	fn real_ping() {
		let ms = ping().unwrap();
		println!("YouTube round trip: {ms} ms");
		assert!(ms < 5000);
	}

	// a real download of "Me at the zoo" (YouTube's first video, 19 s), with the tools in
	// src-tauri/binaries; yt-dlp's own test video (BaW_jenozKc) is unavailable in some regions
	#[test]
	#[ignore]
	fn real_download() {
		let bin = Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
		let dir = std::env::temp_dir().join(format!("moonwave-real-{}", std::process::id()));
		std::fs::create_dir_all(dir.join("bin")).unwrap();
		for name in ["yt-dlp", "ffmpeg", "ffprobe"] {
			std::fs::copy(bin.join(format!("{name}-x86_64-unknown-linux-gnu")), dir.join("bin").join(name)).unwrap();
		}
		// beside_app looks next to the test binary; point PATH at the copies instead
		let path = format!("{}:{}", dir.join("bin").display(), std::env::var("PATH").unwrap_or_default());
		unsafe { std::env::set_var("PATH", path) };
		let tools = Tools { data: dir.join("data") };
		println!("{:?}", info(&tools, &dir));
		let p = preview(&tools, "https://youtu.be/jNQXAC9IVRw").unwrap();
		println!("{p:?}");
		let req = Request { url: p.url.clone(), format: "m4a".into(), title: p.title.clone(), artist: p.artist.clone(), album: String::new(), thumbnail: p.thumbnail.clone() };
		let mut stages = Vec::new();
		let file = download(&tools, &Running::default(), &req, &dir.join("work"), &dir.join("Music"), |pr| stages.push(format!("{} {:.0}", pr.stage, pr.percent))).unwrap();
		println!("{stages:?}\n{}", file.display());
		let read = crate::library::read_track(&file, &dir.join("covers")).unwrap();
		println!("{read:?}");
		assert_eq!(read.title, p.title);
		assert!(read.cover.is_some());
		assert!(read.duration_ms > 15_000);
		std::fs::remove_dir_all(&dir).ok();
	}
}
