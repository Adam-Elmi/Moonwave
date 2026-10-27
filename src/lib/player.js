// ###################
// The player: one <audio> element and the queue. Playing a song from a view queues that whole
// view; shuffle reorders the queue (the playing song stays), repeat is off / all / one.
// Next on the last song goes back to the first (shuffled again); a song downloaded into the
// view that's playing joins what's still coming.
// A song counts as played after 30 s (or half a short one). Volume, shuffle and repeat are
// remembered on this computer. The system media panel follows along (media.rs)
// ###################

import { signal, effect } from "arcmoon/reactive";
import { songUrl, recordPlay, mediaUpdate, onMediaControl } from "./api.js";
import { nextIndex, previousIndex, shuffled, shuffleFrom, unshuffle, countsAsPlay, joinNew } from "./queue.js";

const remembered = (key, fallback) => {
	try {
		const v = localStorage.getItem(`moonwave.${key}`);
		return v === null ? fallback : JSON.parse(v);
	} catch {
		return fallback;
	}
};
const remember = (key, value) => {
	try {
		localStorage.setItem(`moonwave.${key}`, JSON.stringify(value));
	} catch {}
};

export const queue = signal([]);       // songs, in play order
export const index = signal(-1);       // the one playing (or paused) now
export const playing = signal(false);
export const position = signal(0);     // ms
export const length = signal(0);       // ms
export const volume = signal(remembered("volume", 0.8));
export const shuffle = signal(remembered("shuffle", false));
export const repeat = signal(remembered("repeat", "off"));   // off / all / one
export const playError = signal("");
// the song loaded into the player: set when a song loads, so filling or reordering the queue
// never makes it flicker (queue and index change one after the other)
const loaded = signal(null);
export const current = () => loaded();
// the exact position now (ms), for things drawn every frame (position() changes ~4 times a second)
export const nowTime = () => audio.currentTime * 1000;

let original = [];   // the queue before shuffling
let source = null;   // fetches the queue's view again (a download that lands in it joins the queue)
let counted = false;

// Linux plays through GStreamer, whose plugins may be missing (Windows needs nothing)
const linux = navigator.userAgent.includes("Linux");

const audio = new Audio();
audio.preload = "auto";
audio.volume = volume();

// ###################
// Loading and playing
// ###################
let loading = 0;
const load = async (i, play = true) => {
	const song = queue()[i];
	if (!song) return stop();
	index(i);
	loaded(song);
	counted = false;
	playError("");
	position(0);
	length(song.duration_ms ?? 0);
	const mine = ++loading;
	const url = await songUrl(song);
	if (mine !== loading) return;   // another song was chosen meanwhile
	audio.src = url;
	if (play) audio.play().catch(() => {});
};

const stop = () => {
	audio.pause();
	audio.currentTime = 0;
	playing(false);
	position(0);
};

// play `list` starting at song number `start` (the list becomes the queue); `from` fetches
// the view it came from again
export const playList = (list, start = 0, from = null) => {
	if (!list.length) return;
	original = list;
	source = from;
	if (shuffle()) {
		const s = shuffleFrom(list, start);
		queue(s.list);
		load(0);
	} else {
		queue(list);
		load(start);
	}
};

// play a list in random order (turns shuffle on)
export const playShuffled = (list, from = null) => {
	if (!list.length) return;
	shuffle(true);
	remember("shuffle", true);
	playList(list, Math.floor(Math.random() * list.length), from);
};

// the rest of a big view arrives after its song started: `list` becomes the queue around the
// song at `at` (which keeps playing; with shuffle on, the others are shuffled after it)
export const fillQueue = (list, at) => {
	if (current()?.id !== list[at]?.id) return;   // another song was chosen meanwhile
	original = list;
	const s = shuffle() ? shuffleFrom(list, at) : { list, index: at };
	queue(s.list);
	index(s.index);
};

export const toggle = () => {
	if (!current()) return;
	if (audio.paused) audio.play().catch(() => {});
	else audio.pause();
};
export const play = () => current() && audio.paused && audio.play().catch(() => {});
export const pause = () => audio.pause();

export const next = (ended = false) => {
	const i = nextIndex(index(), queue().length, repeat(), ended);
	if (i === -1) return stop();
	if (i === index()) {
		audio.currentTime = 0;
		counted = false;
		return audio.play().catch(() => {});
	}
	// back to the first song: with shuffle on, in a new order (the song just heard isn't first)
	if (i < index() && shuffle()) {
		const list = shuffled(queue());
		if (list[0].id === current()?.id) list.push(list.shift());
		queue(list);
	}
	load(i);
};
export const previous = () => {
	const i = previousIndex(index(), queue().length, position(), repeat());
	if (i === index()) audio.currentTime = 0;
	else load(i);
};
export const seek = (ms) => {
	if (!current()) return;
	audio.currentTime = Math.max(0, Math.min(ms, length() || ms)) / 1000;
	position(audio.currentTime * 1000);
};
export const setVolume = (v) => {
	audio.volume = Math.max(0, Math.min(1, v));
	volume(audio.volume);
	remember("volume", audio.volume);
};
export const toggleShuffle = () => {
	shuffle(!shuffle());
	remember("shuffle", shuffle());
	if (!queue().length) return;
	const s = shuffle() ? shuffleFrom(queue(), index()) : unshuffle(original, current());
	queue(s.list);
	index(s.index);
};
export const cycleRepeat = () => {
	repeat({ off: "all", all: "one", one: "off" }[repeat()] ?? "off");
	remember("repeat", repeat());
};

// ###################
// The queue: play next, add to the end, jump, remove, clear what's coming
// ###################
export const playNext = (song) => {
	if (!current()) return playList([song]);
	const list = [...queue()];
	list.splice(index() + 1, 0, song);
	queue(list);
	original = [...original, song];
};
export const addToQueue = (song) => {
	if (!current()) return playList([song]);
	queue([...queue(), song]);
	original = [...original, song];
};
export const jump = (i) => load(i);
export const removeAt = (i) => {
	if (i === index()) return;
	const list = queue().filter((_, n) => n !== i);
	queue(list);
	if (i < index()) index(index() - 1);
};
export const clearUpcoming = () => queue(queue().slice(0, index() + 1));
// songs added to the queue's view since it started (a download) join what's still coming
export const joinQueue = async () => {
	const from = source;
	if (!from || !current()) return;
	const all = await from().catch(() => null);
	if (!all || from !== source) return;
	const known = new Set(original.map((s) => s.id));
	const fresh = all.filter((s) => !known.has(s.id));
	if (!fresh.length) return;
	const s = joinNew(queue(), index(), fresh, all, shuffle());
	original = joinNew(original, -1, fresh, all, false).list;
	queue(s.list);
	index(s.index);
};
// a deleted song leaves the queue (it's never the one playing)
export const dropSong = (id) => {
	if (current()?.id === id) return;
	const at = index();
	const before = queue().slice(0, at).filter((s) => s.id === id).length;
	queue(queue().filter((s) => s.id !== id));
	original = original.filter((s) => s.id !== id);
	index(at - before);
};

// ###################
// The audio element's events
// ###################
audio.addEventListener("play", () => playing(true));
audio.addEventListener("pause", () => playing(false));
audio.addEventListener("loadedmetadata", () => Number.isFinite(audio.duration) && length(audio.duration * 1000));
audio.addEventListener("timeupdate", () => {
	position(audio.currentTime * 1000);
	const song = current();
	if (song && !counted && countsAsPlay(position(), length())) {
		counted = true;
		recordPlay(song.id).catch(() => {});
	}
});
audio.addEventListener("ended", () => next(true));
audio.addEventListener("error", () => {
	if (!audio.src) return;
	playing(false);
	const ext = (current()?.file.split(".").pop() ?? "").toUpperCase();
	const code = audio.error?.code;
	// 2: the file couldn't be fetched; 3 / 4: it couldn't be decoded
	playError(code === 2 ? `Couldn’t load this ${ext} file (is it still on disk?).` : `Can’t play this ${ext} file (error ${code ?? "?"}${audio.error?.message ? `: ${audio.error.message}` : ""})${linux ? ". For MP3 / M4A, install gst-plugins-good and gst-libav." : "."}`);
});

// ###################
// The system media panel, and its buttons / the media keys
// ###################
effect(() => {
	const song = current();
	const now = { playing: playing(), stopped: !song };
	if (!song) return mediaUpdate({ title: "", artist: "", album: "", cover: null, duration_ms: null, position_ms: 0, ...now });
	mediaUpdate({ title: song.title || "Untitled", artist: song.artist, album: song.album, cover: song.cover, duration_ms: Math.round(length()) || song.duration_ms, position_ms: Math.round(audio.currentTime * 1000), ...now });
});

onMediaControl(({ action, ms }) => {
	if (action === "play") play();
	else if (action === "pause") pause();
	else if (action === "toggle") toggle();
	else if (action === "next") next();
	else if (action === "previous") previous();
	else if (action === "stop") stop();
	else if (action === "seek") seek(position() + (ms ?? 0));
	else if (action === "position") seek(ms ?? 0);
});

// Space plays / pauses (not while typing); Ctrl+→ / Ctrl+← next / previous
window.addEventListener("keydown", (e) => {
	if (e.target.closest?.("input, textarea, select")) return;
	if (e.key === " " && !e.ctrlKey) {
		e.preventDefault();
		toggle();
	} else if (e.ctrlKey && e.key === "ArrowRight") {
		e.preventDefault();
		next();
	} else if (e.ctrlKey && e.key === "ArrowLeft") {
		e.preventDefault();
		previous();
	}
});

// the Now Playing page (open / closed)
export const nowOpen = signal(false);
export const openNow = () => current() && nowOpen(true);
export const closeNow = () => nowOpen(false);

// the queue panel (open / closed)
export const queueOpen = signal(false);
export const toggleQueue = () => queueOpen(!queueOpen());
