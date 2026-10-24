// ###################
// Downloads: the dialog (open / closed), the folder they're saved in (remembered), and the queue.
// One download runs at a time; the others wait. Each item: { key, title, artist, format, status,
// percent, speed, eta, error } with status waiting / running / done / failed / cancelled
// ###################

import { signal } from "arcmoon/reactive";
import { downloadSong, cancelDownload, onDownloadProgress, downloaderInfo } from "./api.js";
import { refreshLibrary, refreshLists, showNote, folders, detail } from "./state.js";

export const downloadOpen = signal(false);
export const openDownload = () => {
	downloadOpen(true);
	refreshInfo();
};
export const closeDownload = () => downloadOpen(false);

export const info = signal(null);
export const refreshInfo = () => downloaderInfo().then(info, () => {});

// the folder downloads go to: the folder open on screen, else the one chosen last time, else the
// first folder that's there ("" → ~/Music/Moonwave, only when the library has no folders)
const SAVE_KEY = "moonwave.saveTo";
const chosen = signal((() => {
	try {
		return localStorage.getItem(SAVE_KEY) ?? "";
	} catch {
		return "";
	}
})());
export const saveTo = () => {
	const there = folders().filter((f) => f.exists);
	const open = detail()?.kind === "folder" ? detail().id : null;
	if (open && there.some((f) => f.id === open)) return open;
	return there.some((f) => f.id === chosen()) ? chosen() : (there[0]?.id ?? "");
};
export const setSaveTo = (id) => {
	chosen(id);
	try {
		localStorage.setItem(SAVE_KEY, id);
	} catch {}
};

export const items = signal([]);
let running = false;
let n = 0;

const change = (key, patch) => items(items().map((it) => (it.key === key ? { ...it, ...patch } : it)));

// request: { url, format, title, artist, album, thumbnail }; folder: a library folder's id; list: an id or null
export const enqueue = (request, folder = "", list = null) => {
	items([...items(), { key: ++n, request, folder, list, title: request.title, artist: request.artist, format: request.format, status: "waiting", percent: 0, speed: "", eta: "", error: "" }]);
	if (!running) run();
};

const run = async () => {
	running = true;
	while (true) {
		const next = items().find((it) => it.status === "waiting");
		if (!next) break;
		change(next.key, { status: "running", stage: "starting" });
		const stop = await onDownloadProgress((p) => change(next.key, { stage: p.stage, percent: p.percent, speed: p.speed, eta: p.eta }));
		try {
			await downloadSong(next.request, next.folder || null, next.list);
			change(next.key, { status: "done", percent: 100 });
			showNote(`“${next.title}” is in your library`);
			refreshLibrary();
			if (next.list) refreshLists();
		} catch (e) {
			const error = String(e?.message ?? e);
			change(next.key, error === "Cancelled." ? { status: "cancelled" } : { status: "failed", error });
		}
		stop();
	}
	running = false;
};

export const cancel = (key) => {
	const it = items().find((i) => i.key === key);
	if (it?.status === "running") cancelDownload();
	else if (it?.status === "waiting") change(key, { status: "cancelled" });
};
export const clearFinished = () => items(items().filter((it) => it.status === "waiting" || it.status === "running"));
export const busy = () => items().some((it) => it.status === "waiting" || it.status === "running");
