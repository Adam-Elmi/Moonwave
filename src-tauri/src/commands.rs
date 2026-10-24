// ###################
// The commands the page may call. Each one is small: the work lives in its own module
// ###################

use crate::db::{Db, Status};
use crate::browse;
use crate::library;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

// where the small cover pictures live (inside the app's data folder)
pub struct CoversDir(pub PathBuf);

#[tauri::command]
pub fn library_status(db: State<Db>) -> Result<Status, String> {
	db.status()
}

// ###################
// Music folders: add (scan), rescan, remove, list. Progress goes to the page as "scan-progress"
// ###################
#[tauri::command]
pub async fn add_folder(app: AppHandle, path: String) -> Result<library::ScanResult, String> {
	run_scan(app, PathBuf::from(path)).await
}

#[tauri::command]
pub async fn rescan_folder(app: AppHandle, id: String) -> Result<library::ScanResult, String> {
	let path = library::path_of(&app.state::<Db>(), &id)?;
	run_scan(app, path).await
}

async fn run_scan(app: AppHandle, folder: PathBuf) -> Result<library::ScanResult, String> {
	tauri::async_runtime::spawn_blocking(move || {
		let db = app.state::<Db>();
		let covers = app.state::<CoversDir>().0.clone();
		let result = library::scan(&db, &covers, &folder, |progress| {
			let _ = app.emit("scan-progress", progress);
		});
		library::allow_in_window(&app, &db);
		result
	})
	.await
	.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_folders(db: State<Db>) -> Result<Vec<library::Folder>, String> {
	library::list(&db)
}

#[tauri::command]
pub fn remove_folder(db: State<Db>, id: String) -> Result<usize, String> {
	library::remove(&db, &id)
}

// ###################
// Browsing (covers come back as full paths, for the window to load)
// ###################
fn full(covers: &CoversDir, cover: &mut Option<String>) {
	*cover = cover.take().map(|c| covers.0.join(c).display().to_string());
}

#[tauri::command]
pub fn list_songs(db: State<Db>, covers: State<CoversDir>, query: browse::SongQuery) -> Result<browse::SongPage, String> {
	let mut page = browse::songs(&db, &query)?;
	page.songs.iter_mut().for_each(|s| full(&covers, &mut s.cover));
	Ok(page)
}

#[tauri::command]
pub fn list_albums(db: State<Db>, covers: State<CoversDir>, search: String, artist: Option<String>) -> Result<Vec<browse::Album>, String> {
	let mut albums = browse::albums(&db, &search, artist.as_deref())?;
	albums.iter_mut().for_each(|a| full(&covers, &mut a.cover));
	Ok(albums)
}

#[tauri::command]
pub fn list_artists(db: State<Db>, covers: State<CoversDir>, search: String) -> Result<Vec<browse::Artist>, String> {
	let mut artists = browse::artists(&db, &search)?;
	artists.iter_mut().for_each(|a| full(&covers, &mut a.cover));
	Ok(artists)
}

// ###################
// Playing: play counts, and what the system media panel shows
// ###################
#[tauri::command]
pub fn record_play(db: State<Db>, id: String) -> Result<(), String> {
	browse::record_play(&db, &id)
}

#[tauri::command]
pub fn media_update(media: State<crate::media::Media>, now: crate::media::NowPlaying) -> Result<(), String> {
	crate::media::update(&media, &now)
}

// ###################
// Lists (covers come back as full paths)
// ###################
#[tauri::command]
pub fn list_lists(db: State<Db>, covers: State<CoversDir>) -> Result<Vec<crate::lists::List>, String> {
	let mut lists = crate::lists::all(&db)?;
	for list in &mut lists {
		list.covers = list.covers.iter().map(|c| covers.0.join(c).display().to_string()).collect();
	}
	Ok(lists)
}

#[tauri::command]
pub fn create_list(db: State<Db>, name: String, tracks: Vec<String>) -> Result<String, String> {
	let id = crate::lists::create(&db, &name)?;
	crate::lists::add(&db, &id, &tracks)?;
	Ok(id)
}

#[tauri::command]
pub fn rename_list(db: State<Db>, id: String, name: String) -> Result<(), String> {
	crate::lists::rename(&db, &id, &name)
}

#[tauri::command]
pub fn delete_list(db: State<Db>, id: String) -> Result<(), String> {
	crate::lists::delete(&db, &id)
}

#[tauri::command]
pub fn add_to_list(db: State<Db>, id: String, tracks: Vec<String>) -> Result<usize, String> {
	crate::lists::add(&db, &id, &tracks)
}

#[tauri::command]
pub fn remove_from_list(db: State<Db>, id: String, position: usize) -> Result<(), String> {
	crate::lists::remove_at(&db, &id, position)
}

#[tauri::command]
pub fn move_in_list(db: State<Db>, id: String, from: usize, to: usize) -> Result<(), String> {
	crate::lists::reorder(&db, &id, from, to)
}

#[tauri::command]
pub fn list_songs_of(db: State<Db>, covers: State<CoversDir>, id: String) -> Result<browse::SongPage, String> {
	let mut songs = crate::lists::songs(&db, &id)?;
	songs.iter_mut().for_each(|s| full(&covers, &mut s.cover));
	Ok(browse::SongPage { total: songs.len() as i64, songs })
}

// ###################
// Downloading (progress goes to the page as "download-progress"); a download lands in the
// library folder you choose (~/Music/Moonwave only when there's none), and can go into a list
// ###################
pub struct DownloadsDir(pub PathBuf);

fn tools(app: &AppHandle) -> crate::download::Tools {
	crate::download::Tools { data: app.path().app_data_dir().unwrap_or_default() }
}

#[tauri::command]
pub async fn downloader_info(app: AppHandle) -> Result<crate::download::Info, String> {
	tauri::async_runtime::spawn_blocking(move || {
		let folder = app.state::<DownloadsDir>().0.clone();
		Ok(crate::download::info(&tools(&app), &folder))
	})
	.await
	.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn download_preview(app: AppHandle, url: String) -> Result<crate::download::Preview, String> {
	tauri::async_runtime::spawn_blocking(move || crate::download::preview(&tools(&app), &url)).await.map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
pub struct Downloaded {
	pub id: String,
	pub file: String,
}

#[tauri::command]
pub async fn download_song(app: AppHandle, request: crate::download::Request, folder: Option<String>, list: Option<String>) -> Result<Downloaded, String> {
	tauri::async_runtime::spawn_blocking(move || {
		let db = app.state::<Db>();
		let (folder_id, folder) = library::download_folder(&db, folder.as_deref(), &app.state::<DownloadsDir>().0)?;
		let work = app.path().app_data_dir().map_err(|e| e.to_string())?.join("downloading");
		let file = crate::download::download(&tools(&app), &app.state::<crate::download::Running>(), &request, &work, &folder, |p| {
			let _ = app.emit("download-progress", p);
		})?;
		// into the library: just this file (the folder isn't scanned again)
		let covers = app.state::<CoversDir>().0.clone();
		let id = library::add_file(&db, &covers, &folder_id, &file).map_err(|e| format!("The song was saved but couldn’t be added to the library ({e})."))?;
		library::allow_in_window(&app, &db);
		let key = file.display().to_string();
		db.conn.lock().map_err(|e| e.to_string())?.execute("UPDATE tracks SET source = 'download', source_url = ?2 WHERE id = ?1", rusqlite::params![id, request.url]).map_err(|e| e.to_string())?;
		if let Some(list) = list.filter(|l| !l.is_empty()) {
			crate::lists::add(&db, &list, std::slice::from_ref(&id))?;
		}
		Ok(Downloaded { id, file: key })
	})
	.await
	.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_download(running: State<crate::download::Running>) {
	crate::download::cancel(&running);
}

#[tauri::command]
pub async fn update_downloader(app: AppHandle) -> Result<String, String> {
	tauri::async_runtime::spawn_blocking(move || crate::download::update(&tools(&app))).await.map_err(|e| e.to_string())?
}

// where the <audio> element gets songs: { base, key } → base + id + "?k=" + key
#[tauri::command]
pub fn stream_info(stream: State<crate::stream::Stream>) -> serde_json::Value {
	serde_json::json!({ "base": stream.base, "key": stream.key })
}

// one song: its file to the Trash, out of the library and every list
#[tauri::command]
pub fn delete_song(db: State<Db>, id: String) -> Result<(), String> {
	library::delete_song(&db, &id, |path| trash::delete(path).map_err(|e| format!("Couldn’t move the file to the Trash ({e}).")))
}

// a song's live bars, analysed once: raw bytes [fps, bands, frames…] (an ArrayBuffer in the page)
#[tauri::command]
pub async fn song_bars(app: AppHandle, id: String) -> Result<tauri::ipc::Response, String> {
	tauri::async_runtime::spawn_blocking(move || {
		let (file, modified): (String, Option<String>) = {
			let db = app.state::<Db>();
			let conn = db.conn.lock().map_err(|e| e.to_string())?;
			conn.query_row("SELECT file, modified FROM tracks WHERE id = ?1", [&id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|_| "That song isn’t in the library.".to_string())?
		};
		let cache = app.path().app_data_dir().map_err(|e| e.to_string())?.join("bars");
		let ffmpeg = tools(&app).ffmpeg_dir();
		crate::spectrum::spectrum(ffmpeg.as_deref(), &cache, &id, modified.as_deref().unwrap_or(""), std::path::Path::new(&file)).map(tauri::ipc::Response::new)
	})
	.await
	.map_err(|e| e.to_string())?
}

// how well YouTube can be reached right now: the round trip in ms, or an error when offline
#[tauri::command]
pub async fn connection_check() -> Result<u64, String> {
	tauri::async_runtime::spawn_blocking(crate::download::ping).await.map_err(|e| e.to_string())?
}
