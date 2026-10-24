// ###################
// The theme: System (follows the desktop), Light or Dark. Remembered on this computer; set on
// <html data-theme> as soon as the page runs, so the page's colors switch with it
// ###################

import { signal } from "arcmoon/reactive";

const KEY = "moonwave.theme";
const MODES = ["system", "light", "dark"];

const saved = (() => {
	try {
		const v = localStorage.getItem(KEY);
		return MODES.includes(v) ? v : "system";
	} catch {
		return "system";
	}
})();

export const theme = signal(saved);

const apply = (mode) => {
	if (mode === "system") document.documentElement.removeAttribute("data-theme");
	else document.documentElement.setAttribute("data-theme", mode);
};
apply(saved);

export const setTheme = (mode) => {
	if (!MODES.includes(mode)) return;
	theme(mode);
	apply(mode);
	try {
		localStorage.setItem(KEY, mode);
	} catch {}
};

// System → Light → Dark → System
export const nextTheme = () => setTheme(MODES[(MODES.indexOf(theme()) + 1) % MODES.length]);
export const THEME_NAMES = { system: "System", light: "Light", dark: "Dark" };
