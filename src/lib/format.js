// ###################
// Small text helpers
// ###################

// 215000 → "3:35", 3725000 → "1:02:05"
export const duration = (ms) => {
	if (ms == null || ms < 0) return "";
	const total = Math.round(ms / 1000);
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const s = String(total % 60).padStart(2, "0");
	return h ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
};

// a path with the home folder as ~ (/home/adam/… on Linux, C:\Users\adam\… on Windows)
export const home = (path) => (path ?? "").replace(/^\/home\/[^/]+|^[A-Za-z]:\\Users\\[^\\]+/, "~");
export const count = (n, one, many) => `${(n ?? 0).toLocaleString()} ${n === 1 ? one : many}`;

// up to two letters for a placeholder: "The Lanterns" → "TL"
export const initials = (name) =>
	(name || "?")
		.split(/\s+/)
		.filter((w) => /\p{L}|\d/u.test(w))
		.slice(0, 2)
		.map((w) => w.match(/\p{L}|\d/u)[0].toUpperCase())
		.join("") || "?";

// the same name always gets the same placeholder color (a hue from 0–359)
export const hue = (name) => {
	let h = 0;
	for (const ch of name || "") h = (h * 31 + ch.codePointAt(0)) % 360;
	return h;
};

// how well YouTube can be reached, from the round trip in ms (null: no answer):
// { level: "good" | "fair" | "weak" | "offline", bars: 0–4, word }
// The bands suit downloads: YouTube is far from some places, so 150–250 ms is a normal, good line
export const signalOf = (ms) => {
	if (ms == null) return { level: "offline", bars: 0, word: "Offline" };
	if (ms < 120) return { level: "good", bars: 4, word: "Good" };
	if (ms < 250) return { level: "good", bars: 3, word: "Good" };
	if (ms < 600) return { level: "fair", bars: 2, word: "Fair" };
	return { level: "weak", bars: 1, word: "Weak" };
};
