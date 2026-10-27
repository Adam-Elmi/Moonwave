// ###################
// The queue's rules, without the audio: what comes next, shuffling, inserting. Plain functions
// on { list, index }, so they can be tested
// ###################

// a shuffled copy (Fisher–Yates); random can be given for tests
export const shuffled = (items, random = Math.random) => {
	const out = [...items];
	for (let i = out.length - 1; i > 0; i--) {
		const j = Math.floor(random() * (i + 1));
		[out[i], out[j]] = [out[j], out[i]];
	}
	return out;
};

// the index after this one: repeat "one" keeps it (only when a song ends by itself),
// "all" wraps around, "off" stops (-1) when the last song ends; pressing Next always wraps
export const nextIndex = (index, length, repeat, ended = false) => {
	if (!length) return -1;
	if (ended && repeat === "one") return index;
	if (index + 1 < length) return index + 1;
	return repeat === "all" || !ended ? 0 : -1;
};

// back: restart the song after 3 seconds, else the one before (or wrap with repeat "all")
export const previousIndex = (index, length, positionMs, repeat) => {
	if (positionMs > 3000 || !length) return index;
	if (index > 0) return index - 1;
	return repeat === "all" ? length - 1 : index;
};

// shuffle on: the playing song stays first, the rest in random order
export const shuffleFrom = (list, index, random) => {
	if (index < 0) return { list: shuffled(list, random), index: -1 };
	const rest = list.filter((_, i) => i !== index);
	return { list: [list[index], ...shuffled(rest, random)], index: 0 };
};

// shuffle off: back to the original order, still on the same song
export const unshuffle = (original, current) => ({ list: original, index: current ? original.findIndex((s) => s.id === current.id) : -1 });

// counts as a play: 30 seconds, or half of a short song
export const countsAsPlay = (positionMs, lengthMs) => positionMs >= Math.min(30_000, (lengthMs || Infinity) / 2);

// new songs (`fresh`, in the order of `all`, the queue's source fetched again) join what's still
// coming: in a random spot with shuffle on, else right after the song before them in `all`.
// The playing song (`index`) stays the same
export const joinNew = (list, index, fresh, all, shuffle, random = Math.random) => {
	const out = [...list];
	const have = new Set(out.map((s) => s.id));
	const order = new Map(all.map((s, i) => [s.id, i]));
	for (const song of fresh) {
		let at;
		if (shuffle) at = index + 1 + Math.floor(random() * (out.length - index));
		else {
			let before = (order.get(song.id) ?? all.length) - 1;
			while (before >= 0 && !have.has(all[before].id)) before--;
			at = before < 0 ? 0 : out.findIndex((s) => s.id === all[before].id) + 1;
		}
		out.splice(at, 0, song);
		have.add(song.id);
		if (at <= index) index++;
	}
	return { list: out, index };
};
