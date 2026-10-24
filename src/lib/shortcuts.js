// ###################
// Keyboard shortcuts: every one Moonwave has, for the Shortcuts dialog (the keyboard button in the
// title bar, or ? / Ctrl+/). Keep this list in step with the key handlers in the components
// ###################

import { signal } from "arcmoon/reactive";
import { startAddFolder, newList } from "./state.js";
import { openDownload } from "./downloads.js";

export const shortcutsOpen = signal(false);
export const openShortcuts = () => shortcutsOpen(true);
export const closeShortcuts = () => shortcutsOpen(false);

// [group, [[keys (each a list of key chips; several = alternatives), what it does], …]]
export const SHORTCUTS = [
	["Playback", [
		[[["Space"]], "Play or pause"],
		[[["Ctrl", "→"]], "Next song"],
		[[["Ctrl", "←"]], "Previous song (restarts it after 3 seconds)"],
		[[["Click"], ["Enter"]], "Play a song and open Now Playing"],
	]],
	["Browsing", [
		[[["Ctrl", "F"]], "Search songs, artists and albums"],
		[[["Enter"]], "Open the selected album, artist or folder"],
		[[["Esc"]], "Close the open page, Now Playing, the queue, a menu or a dialog"],
		[[["Right-click"]], "Song menu: play next, add to a list, delete…"],
	]],
	["Add music", [
		[[["Ctrl", "D"]], "Download a song"],
		[[["Ctrl", "O"]], "Add a music folder"],
		[[["Ctrl", "N"]], "New list"],
	]],
	["Your lists", [
		[[["Alt", "↑"], ["Alt", "↓"]], "Move the selected song up or down"],
		[[["Right-click"]], "Remove Song from List (or × on the row)"],
		[[["Drag"]], "Move a song to another place"],
	]],
	["Seek and volume", [
		[[["←"], ["→"]], "A small step back or forward"],
		[[["Page Up"], ["Page Down"]], "A bigger step"],
		[[["Home"], ["End"]], "The start or the end"],
	]],
	["Window", [
		[[["?"], ["Ctrl", "/"]], "Show these shortcuts"],
		[[["Double-click"]], "The title bar: maximize or restore"],
	]],
];

// ? or Ctrl+/ opens the dialog (not while typing in a field); Ctrl+D / O / N: Download,
// Add Folder, New List (not over another dialog)
const COMMANDS = { d: openDownload, o: startAddFolder, n: () => newList() };
window.addEventListener("keydown", (e) => {
	const command = e.ctrlKey && !e.shiftKey && !e.altKey && COMMANDS[e.key.toLowerCase()];
	if (command) {
		e.preventDefault();
		if (!document.querySelector(".backdrop[data-open], .back[data-open]")) command();
		return;
	}
	if (e.target.closest?.("input, textarea, select")) return;
	if ((e.key === "?" && !e.ctrlKey) || (e.ctrlKey && e.key === "/")) {
		e.preventDefault();
		shortcutsOpen(!shortcutsOpen());
	}
});
