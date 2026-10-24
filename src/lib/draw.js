// ###################
// DOM builders for the browse views (no live [for-each] yet, so rows are drawn by code).
// Their CSS lives in BrowseView.arcm under :global
// ###################

import { fileUrl } from "./api.js";
import { duration, initials, hue, count } from "./format.js";

const el = (tag, className, text) => {
	const node = document.createElement(tag);
	if (className) node.className = className;
	if (text != null) node.textContent = text;
	return node;
};

// a cover picture, or a colored placeholder with the name's initials
export const cover = (src, name, className) => {
	const box = el("div", className);
	box.style.setProperty("--hue", hue(name));
	if (src) {
		const img = el("img");
		img.src = fileUrl(src);
		img.alt = "";
		img.loading = "lazy";
		box.classList.add("has");
		box.append(img);
	} else box.append(el("span", null, initials(name)));
	return box;
};

// one song: its cover (or, inside an album, its track number), title, artist · album, length;
// also artist and album on their own, for the browse view's columns
export const songRow = (song, { numbered = false, index = 0 } = {}) => {
	const li = el("li", "song");
	li.dataset.id = song.id;
	li.tabIndex = 0;
	li.append(numbered ? el("span", "number", song.track_no ?? index + 1) : cover(song.cover, song.album || song.title, "art"));
	const text = el("div", "text");
	text.append(el("b", null, song.title || "Untitled"), el("small", null, numbered ? song.artist || "Unknown artist" : [song.artist || "Unknown artist", song.album].filter(Boolean).join(" · ")));
	// the table columns (the browse view shows these; narrow lists show the line under the title)
	li.append(text, el("span", "artist", song.artist || "Unknown artist"), el("span", "album", song.album || ""), el("span", "length", duration(song.duration_ms)));
	return li;
};

// one album in a grid: cover, name, artist (or the year, on an artist's page)
export const albumCard = (album, { showYear = false } = {}) => {
	const li = el("li", "card");
	li.dataset.album = album.album;
	li.dataset.artist = album.artist;
	li.tabIndex = 0;
	li.title = `${album.album} · ${album.artist}`;
	li.append(cover(album.cover, album.album, "art"), el("b", null, album.album), el("small", null, showYear ? [album.year, count(album.tracks, "song", "songs")].filter(Boolean).join(" · ") : album.artist));
	return li;
};

// one artist: a round picture (a cover from one of their albums), name, counts
export const artistCard = (artist) => {
	const li = el("li", "card person");
	li.dataset.artist = artist.artist;
	li.tabIndex = 0;
	const what = [count(artist.tracks, "song", "songs"), artist.albums ? count(artist.albums, "album", "albums") : ""].filter(Boolean).join(" · ");
	li.append(cover(artist.cover, artist.artist, "art round"), el("b", null, artist.artist), el("small", null, what));
	return li;
};
