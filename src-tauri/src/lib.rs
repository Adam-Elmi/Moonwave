// ###################
// Moonwave: the window, the local database, and the commands the page may call
// Everything you see is the ArcMoon page in ../dist
// ###################

mod browse;
mod commands;
mod db;
mod download;
mod library;
mod lists;
mod media;
mod stream;
mod spectrum;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	tauri::Builder::default()
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_opener::init())
		.setup(|app| {
			let folder = app.path().app_data_dir()?;
			let db = db::Db::open(&folder)?;
			// the window may play files in the music folders, and show the covers
			library::allow_in_window(app.handle(), &db);
			let covers = folder.join("covers");
			std::fs::create_dir_all(&covers)?;
			app.asset_protocol_scope().allow_directory(&covers, false)?;
			app.manage(db);
			app.manage(commands::CoversDir(covers));
			// songs for the <audio> element (WebKitGTK can't stream from asset://)
			app.manage(stream::start(app.handle())?);
			// downloads go to ~/Music/Moonwave when the library has no folder (~/Music even when
			// the system has no registered music folder; never the hidden data folder)
			let music = app.path().audio_dir().or_else(|_| app.path().home_dir().map(|h| h.join("Music"))).unwrap_or_else(|_| folder.join("Music"));
			let downloads = music.join("Moonwave");
			app.manage(commands::DownloadsDir(downloads));
			app.manage(download::Running::default());
			// media keys and the system media panel (none if the system has no media service)
			app.manage(media::Media(std::sync::Mutex::new(media::start(app.handle()))));
			Ok(())
		})
		.invoke_handler(tauri::generate_handler![
			commands::library_status,
			commands::add_folder,
			commands::rescan_folder,
			commands::list_folders,
			commands::remove_folder,
			commands::list_songs,
			commands::list_albums,
			commands::list_artists,
			commands::record_play,
			commands::media_update,
			commands::list_lists,
			commands::create_list,
			commands::rename_list,
			commands::delete_list,
			commands::add_to_list,
			commands::remove_from_list,
			commands::move_in_list,
			commands::list_songs_of,
			commands::downloader_info,
			commands::download_preview,
			commands::download_song,
			commands::cancel_download,
			commands::update_downloader,
			commands::stream_info,
			commands::delete_song,
			commands::song_bars,
			commands::connection_check
		])
		.run(tauri::generate_context!())
		.expect("error while running Moonwave");
}
