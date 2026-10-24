// ###################
// The Rust commands and Tauri plugins, in one place. Components call these, never Tauri directly.
// In a plain browser (no Tauri) inApp is false and calls fail with a clear message
// ###################

const tauri = globalThis.window?.__TAURI__ ?? null;
const invoke = tauri?.core.invoke ?? null;

export const inApp = invoke !== null;

const call = (command, args) => (invoke ? invoke(command, args) : Promise.reject(new Error("Open Moonwave as the desktop app")));

// { tracks, folders, lists, database }
export const libraryStatus = () => call("library_status");

// the system folder picker; the chosen folder's path, or null when cancelled
export const pickFolder = () => (tauri?.dialog ? tauri.dialog.open({ directory: true, multiple: false, title: "Choose a music folder" }) : Promise.resolve(null));

// a URL the window can load a local file from (Tauri's file protocol: music folders and covers only)
export const fileUrl = (path) => (path ? (tauri?.core.convertFileSrc ? tauri.core.convertFileSrc(path) : `file://${path}`) : "");

// ###################
// Music folders. A scan returns { folder, found, added, updated, removed, problems: [{ file, reason }] }
// ###################
export const addFolder = (path) => call("add_folder", { path });
export const rescanFolder = (id) => call("rescan_folder", { id });
// [{ id, name, path, tracks, exists }]
export const listFolders = () => call("list_folders");
// takes the folder's songs out of the library (files stay on disk); returns how many
export const removeFolder = (id) => call("remove_folder", { id });
// progress while scanning: { done, total, file }; returns a function that stops listening
export const onScanProgress = async (handler) => (tauri?.event ? tauri.event.listen("scan-progress", (e) => handler(e.payload)) : () => {});

// ###################
// Browsing
// ###################

// query { search, sort, album, album_artist, artist, offset, limit } →
// { total, songs: [{ id, title, artist, album, album_artist, track_no, duration_ms, cover, file }] }
export const listSongs = (query) => call("list_songs", { query });
// [{ album, artist, year, tracks, duration_ms, cover }]; with artist: the albums they're on
export const listAlbums = (search = "", artist = null) => call("list_albums", { search, artist });
// [{ artist, tracks, albums, cover }]
export const listArtists = (search = "") => call("list_artists", { search });

// ###################
// Playing
// ###################
export const recordPlay = (id) => call("record_play", { id });
// where the <audio> element gets songs: Moonwave's own server on 127.0.0.1 (WebKitGTK can't stream
// over asset://). { base, key }, asked once; outside the app, the file itself
let stream = null;
export const songUrl = async (song) => {
	if (!inApp) return fileUrl(song.file);
	stream ??= call("stream_info");
	const { base, key } = await stream;
	return `${base}${encodeURIComponent(song.id)}?k=${key}`;
};
// now { title, artist, album, cover, duration_ms, position_ms, playing, stopped }: the system media panel
export const mediaUpdate = (now) => call("media_update", { now }).catch(() => {});
// the media keys and the panel's buttons: { action, ms }; returns a function that stops listening
export const onMediaControl = async (handler) => (tauri?.event ? tauri.event.listen("media-control", (e) => handler(e.payload)) : () => {});

// ###################
// Lists
// ###################
// [{ id, name, tracks, duration_ms, covers: [up to 4 paths], updated }]
export const listLists = () => call("list_lists");
// a new list (with these songs in it); returns its id
export const createList = (name, tracks = []) => call("create_list", { name, tracks });
export const renameList = (id, name) => call("rename_list", { id, name });
export const deleteList = (id) => call("delete_list", { id });
// adds songs to the end (those already in are skipped); returns how many were added
export const addToList = (id, tracks) => call("add_to_list", { id, tracks });
export const removeFromList = (id, position) => call("remove_from_list", { id, position });
export const moveInList = (id, from, to) => call("move_in_list", { id, from, to });
// { total, songs } in the list's order (track_no is the position, from 1)
export const listSongsOf = (id) => call("list_songs_of", { id });

// ###################
// Downloading (personal use)
// ###################
// { version, updated, ffmpeg, js_runtime, folder }
export const downloaderInfo = () => call("downloader_info");
// a single video's link → { url, title, artist, album, channel, duration_ms, thumbnail }
export const downloadPreview = (url) => call("download_preview", { url });
// request { url, format: "m4a" | "opus" | "mp3", title, artist, album, thumbnail }; folder: the library
// folder to save in (an id; null → ~/Music/Moonwave); list: an id or null → { id, file } once it's in
export const downloadSong = (request, folder = null, list = null) => call("download_song", { request, folder, list });
export const cancelDownload = () => call("cancel_download");
// fetches the newest yt-dlp; returns its version
export const updateDownloader = () => call("update_downloader");
// how well YouTube can be reached: the round trip in ms (an error when offline)
export const connectionCheck = () => call("connection_check");
// { stage: "starting" | "downloading" | "converting" | "tagging", percent, speed, eta }
export const onDownloadProgress = async (handler) => (tauri?.event ? tauri.event.listen("download-progress", (e) => handler(e.payload)) : () => {});

// one song: its file goes to the Trash, and it leaves the library and every list
export const deleteSong = (id) => call("delete_song", { id });

// a song's live bars (Rust analyses it once, then keeps it): { fps, bands, frames: Uint8Array }
// with `bands` values 0–255 per frame, `fps` frames a second
export const songBars = async (id) => {
	const bytes = new Uint8Array(await call("song_bars", { id }));
	return { fps: bytes[0], bands: bytes[1], frames: bytes.subarray(2) };
};
