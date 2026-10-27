// ###################
// format.js: song lengths and counts
// ###################
import { expect, test } from "vitest";
import { duration, count, initials, hue, signalOf, home } from "../src/lib/format.js";

test("song lengths", () => {
	expect(duration(215000)).toBe("3:35");
	expect(duration(5000)).toBe("0:05");
	expect(duration(3725000)).toBe("1:02:05");
	expect(duration(null)).toBe("");
});

test("counts", () => {
	expect(count(1, "song", "songs")).toBe("1 song");
	expect(count(1200, "song", "songs")).toBe("1,200 songs");
	expect(count(0, "song", "songs")).toBe("0 songs");
});

test("placeholders: initials and a steady color", () => {
	expect(initials("The Lanterns")).toBe("TL");
	expect(initials("ayan")).toBe("A");
	expect(initials("")).toBe("?");
	expect(hue("Coastline")).toBe(hue("Coastline"));
	expect(hue("Coastline")).toBeGreaterThanOrEqual(0);
	expect(hue("Coastline")).toBeLessThan(360);
});

test("connection signal from the round trip", () => {
	expect(signalOf(null)).toEqual({ level: "offline", bars: 0, word: "Offline" });
	expect(signalOf(80).bars).toBe(4);
	expect(signalOf(178)).toEqual({ level: "good", bars: 3, word: "Good" });
	expect(signalOf(400).level).toBe("fair");
	expect(signalOf(900)).toEqual({ level: "weak", bars: 1, word: "Weak" });
});

test("the home folder as ~", () => {
	expect(home("/home/adam/Music/Mixed")).toBe("~/Music/Mixed");
	expect(home("C:\\Users\\adam\\Music")).toBe("~\\Music");
	expect(home("/mnt/music")).toBe("/mnt/music");
	expect(home(null)).toBe("");
});
