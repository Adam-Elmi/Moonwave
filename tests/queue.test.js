// ###################
// queue.js: next / previous with repeat, shuffling, what counts as a play
// ###################
import { expect, test } from "vitest";
import { shuffled, nextIndex, previousIndex, shuffleFrom, unshuffle, countsAsPlay } from "../src/lib/queue.js";

const songs = ["a", "b", "c", "d", "e"].map((id) => ({ id }));
// a fixed "random" so the order is known
const fixed = () => {
	let n = 0;
	return () => [0.1, 0.7, 0.3, 0.9, 0.5][n++ % 5];
};

test("next, with repeat off / all / one", () => {
	expect(nextIndex(1, 5, "off")).toBe(2);
	expect(nextIndex(4, 5, "off")).toBe(-1);
	expect(nextIndex(4, 5, "all")).toBe(0);
	expect(nextIndex(2, 5, "one", true)).toBe(2);
	expect(nextIndex(2, 5, "one", false)).toBe(3);   // pressing Next still moves on
	expect(nextIndex(0, 0, "all")).toBe(-1);
});

test("previous restarts after 3 seconds", () => {
	expect(previousIndex(2, 5, 4000, "off")).toBe(2);
	expect(previousIndex(2, 5, 1000, "off")).toBe(1);
	expect(previousIndex(0, 5, 1000, "off")).toBe(0);
	expect(previousIndex(0, 5, 1000, "all")).toBe(4);
});

test("shuffle keeps every song once, the playing one first; off goes back", () => {
	expect(shuffled(songs, fixed()).map((s) => s.id).sort()).toEqual(["a", "b", "c", "d", "e"]);
	const on = shuffleFrom(songs, 2, fixed());
	expect(on.index).toBe(0);
	expect(on.list[0].id).toBe("c");
	expect(on.list.map((s) => s.id).sort()).toEqual(["a", "b", "c", "d", "e"]);
	const off = unshuffle(songs, on.list[0]);
	expect(off).toEqual({ list: songs, index: 2 });
});

test("a play counts after 30 seconds, or half a short song", () => {
	expect(countsAsPlay(29_000, 240_000)).toBe(false);
	expect(countsAsPlay(30_000, 240_000)).toBe(true);
	expect(countsAsPlay(10_000, 20_000)).toBe(true);
	expect(countsAsPlay(9_000, 20_000)).toBe(false);
});
