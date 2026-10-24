// ###################
// Shared state: components read these signals; ArcMoon bundles one copy for the page,
// so they all see the same values
// ###################

import { signal } from "arcmoon/reactive";
import { inApp, libraryStatus, pickFolder, addFolder, rescanFolder, listFolders, removeFolder, onScanProgress, listSongs, listAlbums, listArtists, listLists, createList, renameList, deleteList, addToList, removeFromList, moveInList, listSongsOf, deleteSong } from "./api.js";

// ###################
// Library status: { tracks, folders, lists, database }, or a problem to show
// ###################
export const status = signal(null);
export const problem = signal("");

export const refreshStatus = async () => {
	if (!inApp) return problem("Open Moonwave as the desktop app");
	try {
		status(await libraryStatus());
		problem("");
	} catch (e) {
		problem(String(e?.message ?? e));
	}
};

// ###################
// The music folders
// ###################
export const folders = signal([]);

// ready: the library has loaded once (the splash waits for it; outside the app there's nothing to wait for)
export const ready = signal(!inApp);
export const refreshLibrary = async () => {
	if (!inApp) return;
	try {
		folders(await listFolders());
	} catch (e) {
		problem(String(e?.message ?? e));
	}
	await refreshStatus();
	refreshLists();
	await reload();
	ready(true);
};

// ###################
// Browsing: a view (songs / albums / artists), or a page for one album or artist on top of it;
// a search for every view, a sort for songs. Songs load a page at a time as you scroll
// ###################
const PAGE = 200;
export const view = signal("songs");
// a page: { kind: "album", album, artist } / { kind: "artist", artist } / { kind: "list", id } /
// { kind: "smart", which: "recent" | "played" } (Recently Added, Most Played: 100 songs each) /
// { kind: "folder", id } (one music folder's songs, A–Z, a page at a time)
export const detail = signal(null);
export const search = signal("");
export const sort = signal("title");

export const songs = signal([]);
export const songTotal = signal(0);
export const loading = signal(false);
export const albums = signal([]);     // the Albums view, or an artist's albums on their page
export const artists = signal([]);

const songQuery = (offset) => {
	const d = detail();
	if (d?.kind === "album") return { album: d.album, album_artist: d.artist, offset, limit: 1000 };
	if (d?.kind === "artist") return { artist: d.artist, sort: "album", offset, limit: 1000 };
	if (d?.kind === "smart") return { sort: d.which, offset, limit: 100 };
	if (d?.kind === "folder") return { folder: d.id, sort: "title", offset, limit: PAGE };
	return { search: search(), sort: sort(), offset, limit: PAGE };
};

let turn = 0;   // an older answer that arrives late is dropped
const loadSongs = async (offset) => {
	const mine = offset === 0 ? ++turn : turn;
	loading(true);
	try {
		const page = detail()?.kind === "list" ? await listSongsOf(detail().id) : await listSongs(songQuery(offset));
		if (mine !== turn) return;
		songTotal(detail()?.kind === "smart" ? page.songs.length : page.total);
		songs(offset === 0 ? page.songs : [...songs(), ...page.songs]);
	} catch (e) {
		problem(String(e?.message ?? e));
	}
	if (mine === turn) loading(false);
};

// what's on screen, loaded again (after a scan, a search, a new view or page)
// fetching: albums or artists on their way (the loading line under the tabs; songs use `loading`)
export const fetching = signal(0);
export const reload = async () => {
	if (!inApp) return;
	const d = detail();
	fetching(fetching() + 1);
	try {
		if (d?.kind === "artist") albums(await listAlbums("", d.artist));
		else if (!d && view() === "albums") albums(await listAlbums(search()));
		else if (!d && view() === "artists") artists(await listArtists(search()));
	} catch (e) {
		problem(String(e?.message ?? e));
	}
	fetching(fetching() - 1);
	if (d || view() === "songs") await loadSongs(0);
};
// every song in what's on screen (the player's queue when you play one of them)
export const songsInView = async () => (detail() && detail().kind !== "folder" ? songs() : (await listSongs({ ...songQuery(0), offset: 0, limit: 100000 })).songs);
export const loadMore = () => {
	if (!loading() && songs().length < songTotal()) loadSongs(songs().length);
};

export const setView = (name) => {
	detail(null);
	view(name);
	reload();
};
export const setSort = (name) => {
	sort(name);
	reload();
};
// typing waits a moment, so a search doesn't run for every letter
let typing = null;
export const setSearch = (text) => {
	search(text);
	clearTimeout(typing);
	typing = setTimeout(reload, 150);
};
export const openAlbum = (album, artist) => {
	detail({ kind: "album", album, artist });
	reload();
};
export const openArtist = (artist) => {
	detail({ kind: "artist", artist });
	reload();
};
export const openList = (id) => {
	detail({ kind: "list", id });
	reload();
};
export const openFolder = (id) => {
	detail({ kind: "folder", id });
	reload();
};
export const openSmart = (which) => {
	detail({ kind: "smart", which });
	reload();
};
export const closeDetail = () => {
	detail(null);
	reload();
};

// ###################
// Scanning: "idle" → "running" (progress) → "done" (the result) → "idle"
// ###################
export const scanStage = signal("idle");
export const scanProgress = signal({ done: 0, total: 0, file: "" });
export const scanResult = signal(null);

const runScan = async (work) => {
	scanStage("running");
	scanProgress({ done: 0, total: 0, file: "" });
	const stop = await onScanProgress(scanProgress);
	try {
		scanResult(await work());
	} catch (e) {
		scanResult({ error: String(e?.message ?? e) });
	}
	stop();
	scanStage("done");
	refreshLibrary();
};

export const startAddFolder = async () => {
	if (scanStage() === "running") return;
	const path = await pickFolder();
	if (path) runScan(() => addFolder(path));
};
export const rescan = (id) => scanStage() !== "running" && runScan(() => rescanFolder(id));
export const closeScan = () => scanStage("idle");

// ###################
// Confirming: { title, body, label, then } while the alert is open
// ###################
export const confirming = signal(null);
export const answerConfirm = (yes) => {
	const ask = confirming();
	confirming(null);
	if (yes) ask?.then();
};
export const askRemoveFolder = (id) => {
	const folder = folders().find((f) => f.id === id);
	if (!folder) return;
	confirming({
		title: `Remove “${folder.name}”?`,
		body: `Its ${folder.tracks.toLocaleString()} ${folder.tracks === 1 ? "song leaves" : "songs leave"} the library and your lists. The files stay on your computer.`,
		label: "Remove",
		then: async () => {
			try {
				await removeFolder(id);
			} catch (e) {
				problem(String(e?.message ?? e));
			}
			refreshLibrary();
		}
	});
};

// ###################
// Lists: the sidebar's lists, the list dialog ({ mode: "new" | "rename" | "pick", tracks, id, name }),
// and a short note after an action ("Added 3 songs to Night drive")
// ###################
export const lists = signal([]);
export const listDialog = signal(null);
export const note = signal("");
let noteTimer = null;
export const showNote = (text) => {
	note(text);
	clearTimeout(noteTimer);
	noteTimer = setTimeout(() => note(""), 2600);
};

export const refreshLists = async () => {
	if (!inApp) return;
	try {
		lists(await listLists());
	} catch (e) {
		problem(String(e?.message ?? e));
	}
};
const afterListChange = async (id) => {
	await refreshLists();
	if (detail()?.kind === "list" && detail().id === id) reload();
};
const songsText = (n) => `${n.toLocaleString()} ${n === 1 ? "song" : "songs"}`;

export const newList = (tracks = []) => listDialog({ mode: "new", tracks });
export const pickList = (tracks) => listDialog({ mode: "pick", tracks });
export const askRename = (id) => listDialog({ mode: "rename", id, name: lists().find((l) => l.id === id)?.name ?? "" });
export const closeListDialog = () => listDialog(null);

// the dialog's answers; each returns an error message to show, or "" when done
export const makeList = async (name, tracks) => {
	try {
		const id = await createList(name, tracks);
		await refreshLists();
		showNote(tracks.length ? `Made “${name.trim()}” with ${songsText(tracks.length)}` : `Made “${name.trim()}”`);
		if (!tracks.length) openList(id);
		return "";
	} catch (e) {
		return String(e?.message ?? e);
	}
};
export const putInList = async (id, tracks) => {
	try {
		const added = await addToList(id, tracks);
		const name = lists().find((l) => l.id === id)?.name ?? "the list";
		showNote(added ? `Added ${songsText(added)} to “${name}”` : `Already in “${name}”`);
		await afterListChange(id);
		return "";
	} catch (e) {
		return String(e?.message ?? e);
	}
};
export const nameList = async (id, name) => {
	try {
		await renameList(id, name);
		await afterListChange(id);
		return "";
	} catch (e) {
		return String(e?.message ?? e);
	}
};
export const takeFromList = async (id, position) => {
	await removeFromList(id, position).catch((e) => problem(String(e?.message ?? e)));
	afterListChange(id);
};
export const moveSong = async (id, from, to) => {
	await moveInList(id, from, to).catch((e) => problem(String(e?.message ?? e)));
	afterListChange(id);
};
export const askDeleteList = (id) => {
	const list = lists().find((l) => l.id === id);
	if (!list) return;
	confirming({
		title: `Delete “${list.name}”?`,
		body: "The list goes; its songs stay in your library.",
		label: "Delete",
		then: async () => {
			await deleteList(id).catch((e) => problem(String(e?.message ?? e)));
			if (detail()?.kind === "list" && detail().id === id) closeDetail();
			refreshLists();
		}
	});
};

// ###################
// Deleting one song (never the one playing): asks first; the file goes to the Trash
// ###################
export const askDeleteSong = (song, isPlaying, removeFromQueue) => {
	if (!song || isPlaying) return;
	confirming({
		title: `Delete “${song.title || "Untitled"}”?`,
		body: "The file goes to the Trash, and the song leaves your library and your lists.",
		label: "Delete",
		then: async () => {
			try {
				await deleteSong(song.id);
				removeFromQueue(song.id);
				showNote(`Deleted “${song.title || "Untitled"}”`);
			} catch (e) {
				problem(String(e?.message ?? e));
				showNote(String(e?.message ?? e));
			}
			refreshLibrary();
		}
	});
};

refreshLibrary();
