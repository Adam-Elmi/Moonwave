// ###################
// The library: music folders and the songs in them. Scanning walks a folder, reads each audio
// file's tags (lofty) and cover, and keeps the tracks table in step with the files: new files are
// added, changed ones read again, missing ones removed. Unchanged files (same time) are skipped.
// Removing a folder only removes its songs from the library: files are never touched
// ###################

use crate::db::{iso, now, Db};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::tag::{Accessor, ItemKey, Tag};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// the audio files a scan picks up
const AUDIO: [&str; 10] = ["mp3", "m4a", "aac", "opus", "ogg", "oga", "flac", "wav", "aiff", "wma"];
// covers are kept as small WebP pictures, one per distinct picture
const COVER_SIZE: u32 = 320;

#[derive(Serialize, Debug, PartialEq)]
pub struct Folder {
	pub id: String,
	pub name: String,
	pub path: String,
	pub tracks: i64,
	pub exists: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct Progress {
	pub done: usize,
	pub total: usize,
	pub file: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Problem {
	pub file: String,
	pub reason: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ScanResult {
	pub folder: String,
	pub found: usize,
	pub added: usize,
	pub updated: usize,
	pub removed: usize,
	pub problems: Vec<Problem>,
}

// what one file's tags say
#[derive(Debug, Default, PartialEq)]
pub struct TrackInfo {
	pub title: String,
	pub artist: String,
	pub album: String,
	pub album_artist: String,
	pub track_no: Option<u32>,
	pub disc_no: Option<u32>,
	pub year: Option<u16>,
	pub duration_ms: i64,
	pub cover: Option<String>,
}

// ###################
// Folders
// ###################
pub fn list(db: &Db) -> Result<Vec<Folder>, String> {
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let mut stmt = conn
		.prepare(
			"SELECT f.id, f.name, f.path, COUNT(t.id)
			 FROM folders f LEFT JOIN tracks t ON t.folder_id = f.id
			 GROUP BY f.id ORDER BY f.name COLLATE NOCASE",
		)
		.map_err(|e| e.to_string())?;
	let folders = stmt
		.query_map([], |r| {
			let path: String = r.get(2)?;
			Ok(Folder { id: r.get(0)?, name: r.get(1)?, exists: Path::new(&path).is_dir(), path, tracks: r.get(3)? })
		})
		.map_err(|e| e.to_string())?
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| e.to_string())?;
	Ok(folders)
}

// let the window play files in every music folder, and nowhere else
pub fn allow_in_window(app: &tauri::AppHandle, db: &Db) {
	use tauri::Manager;
	let Ok(folders) = list(db) else { return };
	let scope = app.asset_protocol_scope();
	for folder in folders.iter().filter(|f| f.exists) {
		let _ = scope.allow_directory(&folder.path, true);
	}
}

pub fn path_of(db: &Db, id: &str) -> Result<PathBuf, String> {
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	conn.query_row("SELECT path FROM folders WHERE id = ?1", [id], |r| r.get::<_, String>(0))
		.optional()
		.map_err(|e| e.to_string())?
		.map(PathBuf::from)
		.ok_or_else(|| "That folder isn’t in the library any more.".into())
}

// takes the folder's songs out of the library (and out of every list); returns how many
pub fn remove(db: &Db, id: &str) -> Result<usize, String> {
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let songs = conn.execute("DELETE FROM tracks WHERE folder_id = ?1", [id]).map_err(|e| e.to_string())?;
	conn.execute("DELETE FROM folders WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
	Ok(songs)
}

fn ensure_folder(db: &Db, path: &Path) -> Result<String, String> {
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	let key = path.display().to_string();
	if let Some(id) = conn.query_row("SELECT id FROM folders WHERE path = ?1", [&key], |r| r.get::<_, String>(0)).optional().map_err(|e| e.to_string())? {
		return Ok(id);
	}
	let id = uuid::Uuid::new_v4().to_string();
	let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Music").to_string();
	conn.execute("INSERT INTO folders (id, path, name, added) VALUES (?1, ?2, ?3, ?4)", params![id, key, name, now()]).map_err(|e| e.to_string())?;
	Ok(id)
}

// ###################
// Scanning
// ###################

// every audio file under a folder, sorted; hidden folders and files are skipped
pub fn audio_files(folder: &Path) -> Vec<PathBuf> {
	let mut found = Vec::new();
	let mut stack = vec![folder.to_path_buf()];
	while let Some(dir) = stack.pop() {
		let Ok(entries) = std::fs::read_dir(&dir) else { continue };
		for entry in entries.flatten() {
			let path = entry.path();
			if entry.file_name().to_string_lossy().starts_with('.') {
				continue;
			}
			if path.is_dir() {
				stack.push(path);
			} else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| AUDIO.contains(&e.to_ascii_lowercase().as_str())) {
				found.push(path);
			}
		}
	}
	found.sort();
	found
}

// the file's modified time, to tell whether it changed since the last scan
fn modified(path: &Path) -> Option<String> {
	let time = std::fs::metadata(path).ok()?.modified().ok()?;
	Some(iso(time.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64))
}

pub fn scan(db: &Db, covers: &Path, folder: &Path, mut on_progress: impl FnMut(Progress)) -> Result<ScanResult, String> {
	if !folder.is_dir() {
		return Err(format!("“{}” isn’t a folder, or it can’t be opened.", folder.display()));
	}
	let folder_id = ensure_folder(db, folder)?;
	let files = audio_files(folder);
	// what the library already has for this folder: file → (id, modified)
	let known: HashMap<String, (String, Option<String>)> = {
		let conn = db.conn.lock().map_err(|e| e.to_string())?;
		let mut stmt = conn.prepare("SELECT file, id, modified FROM tracks WHERE folder_id = ?1").map_err(|e| e.to_string())?;
		stmt.query_map([&folder_id], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))
			.map_err(|e| e.to_string())?
			.collect::<Result<_, _>>()
			.map_err(|e| e.to_string())?
	};
	let mut result = ScanResult { folder: folder.display().to_string(), found: files.len(), added: 0, updated: 0, removed: 0, problems: Vec::new() };
	let total = files.len();
	for (i, path) in files.iter().enumerate() {
		let file = path.display().to_string();
		on_progress(Progress { done: i, total, file: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() });
		let time = modified(path);
		let existing = known.get(&file);
		if existing.is_some_and(|(_, t)| *t == time && time.is_some()) {
			continue;
		}
		// tags are read without holding the database
		let info = match read_track(path, covers) {
			Ok(info) => info,
			Err(reason) => {
				result.problems.push(Problem { file: file.clone(), reason });
				continue;
			}
		};
		let size = std::fs::metadata(path).map(|m| m.len() as i64).ok();
		let conn = db.conn.lock().map_err(|e| e.to_string())?;
		let values = params![info.title, info.artist, info.album, info.album_artist, info.track_no, info.disc_no, info.year, info.duration_ms, info.cover, size, time];
		match existing {
			Some((id, _)) => {
				conn.execute(
					"UPDATE tracks SET title = ?1, artist = ?2, album = ?3, album_artist = ?4, track_no = ?5, disc_no = ?6, year = ?7,
					 duration_ms = ?8, cover = ?9, size = ?10, modified = ?11 WHERE id = ?12",
					rusqlite::params_from_iter(values.iter().copied().chain([id as &dyn rusqlite::ToSql])),
				)
				.map_err(|e| e.to_string())?;
				result.updated += 1;
			}
			None => {
				let id = uuid::Uuid::new_v4().to_string();
				let added = now();
				conn.execute(
					"INSERT INTO tracks (title, artist, album, album_artist, track_no, disc_no, year, duration_ms, cover, size, modified, id, folder_id, file, added)
					 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
					rusqlite::params_from_iter(values.iter().copied().chain([&id as &dyn rusqlite::ToSql, &folder_id, &file, &added])),
				)
				.map_err(|e| e.to_string())?;
				result.added += 1;
			}
		}
	}
	// files that are gone: out of the library
	let present: std::collections::HashSet<String> = files.iter().map(|p| p.display().to_string()).collect();
	{
		let conn = db.conn.lock().map_err(|e| e.to_string())?;
		for (id, _) in known.iter().filter(|(f, _)| !present.contains(*f)).map(|(_, v)| v) {
			conn.execute("DELETE FROM tracks WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
			result.removed += 1;
		}
	}
	on_progress(Progress { done: total, total, file: String::new() });
	Ok(result)
}

// deletes one song: its file goes to the Trash (`discard`; nothing happens to the library if
// that fails) and it leaves the library and every list. A file that's already gone just leaves
pub fn delete_song(db: &Db, id: &str, discard: impl Fn(&Path) -> Result<(), String>) -> Result<(), String> {
	let file: Option<String> = {
		let conn = db.conn.lock().map_err(|e| e.to_string())?;
		conn.query_row("SELECT file FROM tracks WHERE id = ?1", [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?
	};
	let Some(file) = file else { return Err("That song isn’t in the library any more.".into()) };
	let path = Path::new(&file);
	if path.exists() {
		discard(path)?;
	}
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	conn.execute("DELETE FROM tracks WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
	Ok(())
}

// one new file (a download) into a folder that's already in the library, without scanning the
// whole folder; returns the song's id
pub fn add_file(db: &Db, covers: &Path, folder_id: &str, path: &Path) -> Result<String, String> {
	let info = read_track(path, covers)?;
	let size = std::fs::metadata(path).map(|m| m.len() as i64).ok();
	let id = uuid::Uuid::new_v4().to_string();
	let conn = db.conn.lock().map_err(|e| e.to_string())?;
	conn.execute(
		"INSERT INTO tracks (id, folder_id, file, title, artist, album, album_artist, track_no, disc_no, year, duration_ms, cover, size, modified, added)
		 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
		params![id, folder_id, path.display().to_string(), info.title, info.artist, info.album, info.album_artist, info.track_no, info.disc_no, info.year, info.duration_ms, info.cover, size, modified(path), now()],
	)
	.map_err(|e| e.to_string())?;
	Ok(id)
}

// the folder a download goes to: the one chosen (it must still be there), else the default
// one, which is added to the library the first time; returns (folder id, path)
pub fn download_folder(db: &Db, chosen: Option<&str>, fallback: &Path) -> Result<(String, PathBuf), String> {
	if let Some(id) = chosen.filter(|id| !id.is_empty()) {
		let path = path_of(db, id)?;
		if !path.is_dir() {
			return Err(format!("“{}” can’t be found. Plug it in, or choose another folder.", path.display()));
		}
		return Ok((id.to_string(), path));
	}
	std::fs::create_dir_all(fallback).map_err(|e| e.to_string())?;
	Ok((ensure_folder(db, fallback)?, fallback.to_path_buf()))
}

// ###################
// One file: its tags, length and cover. No title → the file name; no artist / album → empty
// ###################
pub fn read_track(path: &Path, covers: &Path) -> Result<TrackInfo, String> {
	let tagged = lofty::read_from_path(path).map_err(|e| format!("couldn’t read it as audio ({e})"))?;
	let mut info = TrackInfo { duration_ms: tagged.properties().duration().as_millis() as i64, ..Default::default() };
	let tag: Option<&Tag> = tagged.primary_tag().or_else(|| tagged.first_tag());
	if let Some(tag) = tag {
		let text = |v: Option<std::borrow::Cow<'_, str>>| v.map(|s| s.trim().to_string()).unwrap_or_default();
		info.title = text(tag.title());
		info.artist = text(tag.artist());
		info.album = text(tag.album());
		info.album_artist = tag.get_string(ItemKey::AlbumArtist).map(|s| s.trim().to_string()).unwrap_or_default();
		info.track_no = tag.track();
		info.disc_no = tag.disk();
		// a full date, or just "2019"
		info.year = tag.date().map(|d| d.year).or_else(|| {
			[ItemKey::RecordingDate, ItemKey::Year, ItemKey::OriginalReleaseDate]
				.into_iter()
				.find_map(|key| tag.get_string(key).and_then(|v| v.trim().get(..4)?.parse().ok()))
		});
		let picture = tag.pictures().iter().find(|p| p.pic_type() == PictureType::CoverFront).or_else(|| tag.pictures().first());
		if let Some(picture) = picture {
			info.cover = save_cover(picture.data(), covers).ok();
		}
	}
	if info.title.is_empty() {
		info.title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
	}
	Ok(info)
}

// a cover picture → a small WebP named by its content, so an album's songs share one file;
// returns the file name (inside the covers folder)
pub fn save_cover(bytes: &[u8], covers: &Path) -> Result<String, String> {
	let hash: String = Sha256::digest(bytes).iter().take(12).map(|b| format!("{b:02x}")).collect();
	let name = format!("{hash}.webp");
	let target = covers.join(&name);
	if target.exists() {
		return Ok(name);
	}
	let picture = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
	let small = picture.thumbnail(COVER_SIZE, COVER_SIZE).to_rgba8();
	let encoded = webp::Encoder::from_rgba(&small, small.width(), small.height()).encode(80.0);
	std::fs::create_dir_all(covers).map_err(|e| e.to_string())?;
	std::fs::write(&target, &*encoded).map_err(|e| e.to_string())?;
	Ok(name)
}

// ###################
// Tests: real (tiny) WAV files, tagged with lofty, scanned into a fresh library
// ###################
#[cfg(test)]
mod tests {
	use super::*;
	use lofty::config::WriteOptions;
	use lofty::picture::{MimeType, Picture};
	use lofty::tag::TagType;

	fn temp(name: &str) -> PathBuf {
		let folder = std::env::temp_dir().join(format!("moonwave-lib-{name}-{}", std::process::id()));
		std::fs::remove_dir_all(&folder).ok();
		std::fs::create_dir_all(&folder).unwrap();
		folder
	}

	// one second of silence, 8 kHz mono 16-bit
	fn wav(path: &Path) {
		let samples = 8000u32;
		let data = samples * 2;
		let mut b = Vec::new();
		b.extend(b"RIFF");
		b.extend((36 + data).to_le_bytes());
		b.extend(b"WAVEfmt ");
		b.extend(16u32.to_le_bytes());
		b.extend(1u16.to_le_bytes());
		b.extend(1u16.to_le_bytes());
		b.extend(8000u32.to_le_bytes());
		b.extend(16000u32.to_le_bytes());
		b.extend(2u16.to_le_bytes());
		b.extend(16u16.to_le_bytes());
		b.extend(b"data");
		b.extend(data.to_le_bytes());
		b.extend(vec![0u8; data as usize]);
		std::fs::write(path, b).unwrap();
	}

	fn png() -> Vec<u8> {
		let mut out = std::io::Cursor::new(Vec::new());
		image::RgbImage::from_pixel(64, 64, image::Rgb([200, 40, 90])).write_to(&mut out, image::ImageFormat::Png).unwrap();
		out.into_inner()
	}

	fn tagged(path: &Path, title: &str, artist: &str, album: &str, track: u32, cover: bool) {
		wav(path);
		let mut file = lofty::read_from_path(path).unwrap();
		let mut tag = Tag::new(TagType::Id3v2);
		tag.set_title(title.into());
		tag.set_artist(artist.into());
		tag.set_album(album.into());
		tag.set_track(track);
		tag.insert_text(ItemKey::RecordingDate, "2019-05-01".into());
		if cover {
			tag.push_picture(Picture::unchecked(png()).pic_type(PictureType::CoverFront).mime_type(MimeType::Png).build());
		}
		file.insert_tag(tag);
		file.save_to_path(path, WriteOptions::default()).unwrap();
	}

	fn titles(db: &Db) -> Vec<(String, String, Option<i64>)> {
		let conn = db.conn.lock().unwrap();
		let mut stmt = conn.prepare("SELECT title, artist, track_no FROM tracks ORDER BY title").unwrap();
		stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
	}

	#[test]
	fn reads_tags_length_and_cover() {
		let dir = temp("read");
		let covers = dir.join("covers");
		let song = dir.join("song.wav");
		tagged(&song, "Moonrise", "Nadia", "Night Songs", 3, true);
		let info = read_track(&song, &covers).unwrap();
		assert_eq!((info.title.as_str(), info.artist.as_str(), info.album.as_str()), ("Moonrise", "Nadia", "Night Songs"));
		assert_eq!((info.track_no, info.year), (Some(3), Some(2019)));
		assert!((990..=1010).contains(&info.duration_ms), "about one second: {}", info.duration_ms);
		let cover = info.cover.expect("a cover");
		assert!(covers.join(&cover).exists());
		// the same picture again: the same file
		let other = dir.join("other.wav");
		tagged(&other, "Tide", "Nadia", "Night Songs", 4, true);
		assert_eq!(read_track(&other, &covers).unwrap().cover.unwrap(), cover);
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn no_tags_gives_the_file_name() {
		let dir = temp("bare");
		let song = dir.join("Untagged Song.wav");
		wav(&song);
		let info = read_track(&song, &dir.join("covers")).unwrap();
		assert_eq!((info.title.as_str(), info.artist.as_str(), info.cover), ("Untagged Song", "", None));
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn deleting_one_song() {
		let dir = temp("delete");
		let music = dir.join("Music");
		std::fs::create_dir_all(&music).unwrap();
		tagged(&music.join("keep.wav"), "Keep", "A", "X", 1, false);
		tagged(&music.join("bin.wav"), "Bin", "A", "X", 2, false);
		let db = Db::open(&dir.join("data")).unwrap();
		scan(&db, &dir.join("covers"), &music, |_| {}).unwrap();
		let id_of = |title: &str| db.conn.lock().unwrap().query_row("SELECT id FROM tracks WHERE title = ?1", [title], |r| r.get::<_, String>(0)).unwrap();
		let bin = id_of("Bin");
		let list = crate::lists::create(&db, "Mine").unwrap();
		crate::lists::add(&db, &list, &[bin.clone(), id_of("Keep")]).unwrap();
		// the Trash refuses: nothing changes
		assert!(delete_song(&db, &bin, |_| Err("no trash".into())).is_err());
		assert_eq!(titles(&db).len(), 2);
		// it works: the file is handed over, the song leaves the library and the list, the other stays
		let gone = std::cell::RefCell::new(Vec::new());
		delete_song(&db, &bin, |p| {
			gone.borrow_mut().push(p.to_path_buf());
			std::fs::remove_file(p).map_err(|e| e.to_string())
		})
		.unwrap();
		assert_eq!(*gone.borrow(), vec![music.join("bin.wav")]);
		assert_eq!(titles(&db), vec![("Keep".into(), "A".into(), Some(1))]);
		assert_eq!(crate::lists::songs(&db, &list).unwrap().len(), 1);
		assert!(music.join("keep.wav").exists());
		assert!(delete_song(&db, &bin, |_| Ok(())).is_err(), "already gone");
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn a_download_goes_into_the_chosen_folder() {
		let dir = temp("download");
		let music = dir.join("Music");
		std::fs::create_dir_all(&music).unwrap();
		tagged(&music.join("old.wav"), "Old", "A", "X", 1, false);
		let db = Db::open(&dir.join("data")).unwrap();
		let covers = dir.join("covers");
		scan(&db, &covers, &music, |_| {}).unwrap();
		let id = list(&db).unwrap()[0].id.clone();
		// the chosen folder
		let (folder, path) = download_folder(&db, Some(&id), &dir.join("Fallback")).unwrap();
		assert_eq!((folder.as_str(), path.as_path()), (id.as_str(), music.as_path()));
		tagged(&music.join("new.wav"), "New", "B", "", 0, false);
		add_file(&db, &covers, &folder, &music.join("new.wav")).unwrap();
		assert_eq!(list(&db).unwrap()[0].tracks, 2);
		assert!(!dir.join("Fallback").exists(), "no extra folder");
		// a folder that's gone: an error, not a new folder
		let gone = dir.join("Gone");
		std::fs::create_dir_all(&gone).unwrap();
		scan(&db, &covers, &gone, |_| {}).unwrap();
		let gone_id = list(&db).unwrap().into_iter().find(|f| f.name == "Gone").unwrap().id;
		std::fs::remove_dir_all(&gone).unwrap();
		assert!(download_folder(&db, Some(&gone_id), &dir.join("Fallback")).unwrap_err().contains("can’t be found"));
		// nothing chosen: the fallback, added once
		let (first, _) = download_folder(&db, None, &dir.join("Fallback")).unwrap();
		let (again, _) = download_folder(&db, None, &dir.join("Fallback")).unwrap();
		assert_eq!(first, again);
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn scanning_adds_skips_updates_and_removes() {
		let dir = temp("scan");
		let music = dir.join("Music");
		std::fs::create_dir_all(music.join("Album")).unwrap();
		std::fs::create_dir_all(music.join(".hidden")).unwrap();
		tagged(&music.join("Album/01.wav"), "One", "A", "Album", 1, false);
		tagged(&music.join("Album/02.wav"), "Two", "A", "Album", 2, false);
		wav(&music.join(".hidden/secret.wav"));
		std::fs::write(music.join("notes.txt"), "not audio").unwrap();
		std::fs::write(music.join("broken.mp3"), "not really an mp3").unwrap();
		let db = Db::open(&dir.join("data")).unwrap();
		let covers = dir.join("covers");

		let mut seen = Vec::new();
		let first = scan(&db, &covers, &music, |p| seen.push(p.done)).unwrap();
		assert_eq!((first.found, first.added, first.updated, first.removed), (3, 2, 0, 0));
		assert_eq!(first.problems.len(), 1, "the fake mp3");
		assert_eq!(seen.last(), Some(&3));
		assert_eq!(titles(&db), vec![("One".into(), "A".into(), Some(1)), ("Two".into(), "A".into(), Some(2))]);

		// nothing changed: nothing read again
		let again = scan(&db, &covers, &music, |_| {}).unwrap();
		assert_eq!((again.added, again.updated, again.removed), (0, 0, 0));

		// one retagged (a newer file time), one deleted
		std::thread::sleep(std::time::Duration::from_millis(1100));
		tagged(&music.join("Album/01.wav"), "One (Live)", "A", "Album", 1, false);
		std::fs::remove_file(music.join("Album/02.wav")).unwrap();
		let later = scan(&db, &covers, &music, |_| {}).unwrap();
		assert_eq!((later.added, later.updated, later.removed), (0, 1, 1));
		assert_eq!(titles(&db), vec![("One (Live)".into(), "A".into(), Some(1))]);

		// the folder is listed once, with its count; removing it empties the library
		let folders = list(&db).unwrap();
		assert_eq!((folders.len(), folders[0].name.as_str(), folders[0].tracks), (1, "Music", 1));
		assert_eq!(remove(&db, &folders[0].id).unwrap(), 1);
		assert_eq!(db.status().unwrap().tracks, 0);
		std::fs::remove_dir_all(&dir).ok();
	}
}
