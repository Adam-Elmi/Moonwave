// ###################
// The live bars: a song's analysed sound (songBars) played back in step with the player.
// Each frame it reads the two snapshots around the exact position, blends them, and eases the
// bars (up quickly, down slowly) so they move like a meter. It draws only while it's shown and
// the song plays; when paused the bars sink and the drawing stops
// ###################

export const makeVisualizer = (canvas, { time, active }) => {
	let data = null;
	let heights = [];
	let frame = 0;

	const sizes = () => {
		const ratio = window.devicePixelRatio || 1;
		const w = canvas.clientWidth;
		const h = canvas.clientHeight;
		if (canvas.width !== Math.round(w * ratio) || canvas.height !== Math.round(h * ratio)) {
			canvas.width = Math.round(w * ratio);
			canvas.height = Math.round(h * ratio);
		}
		return { w, h, ratio };
	};

	// the bands at a time (ms), blended between the two snapshots around it
	const at = (ms) => {
		const { fps, bands, frames } = data;
		const count = frames.length / bands;
		const f = Math.min(count - 1, Math.max(0, (ms / 1000) * fps));
		const a = Math.floor(f);
		const b = Math.min(count - 1, a + 1);
		const mix = f - a;
		return Array.from({ length: bands }, (_, i) => (frames[a * bands + i] * (1 - mix) + frames[b * bands + i] * mix) / 255);
	};

	const draw = () => {
		frame = 0;
		if (!data) return;
		const { w, h, ratio } = sizes();
		if (!w || !h) return;
		const on = active();
		const target = on ? at(time()) : null;
		let moving = false;
		heights = Array.from({ length: data.bands }, (_, i) => {
			const now = heights[i] ?? 0;
			const want = target ? target[i] : 0;
			const next = want > now ? now + (want - now) * 0.6 : now + (want - now) * 0.12;
			if (Math.abs(next - want) > 0.004) moving = true;
			return next;
		});
		const g = canvas.getContext("2d");
		g.setTransform(ratio, 0, 0, ratio, 0, 0);
		g.clearRect(0, 0, w, h);
		const css = getComputedStyle(canvas);
		const color = css.getPropertyValue("--accent").trim() || "#c2255c";
		const gap = 4;
		const bar = Math.max(3, (w - gap * (data.bands - 1)) / data.bands);
		const left = (w - (bar * data.bands + gap * (data.bands - 1))) / 2;
		g.fillStyle = color;
		heights.forEach((v, i) => {
			const bh = Math.max(3, v * h);
			const x = left + i * (bar + gap);
			g.globalAlpha = 0.45 + 0.55 * v;
			g.beginPath();
			g.roundRect(x, h - bh, bar, bh, Math.min(2, bar / 2));
			g.fill();
		});
		g.globalAlpha = 1;
		// keep going while it plays, or until the bars have settled
		if (on || moving) frame = requestAnimationFrame(draw);
	};

	const wake = () => {
		if (!frame) frame = requestAnimationFrame(draw);
	};
	new ResizeObserver(wake).observe(canvas);
	document.addEventListener("visibilitychange", wake);

	return {
		show: (bars) => {
			data = bars && bars.frames.length ? bars : null;
			heights = [];
			canvas.toggleAttribute("data-empty", data === null);
			if (!data) canvas.getContext("2d").clearRect(0, 0, canvas.width, canvas.height);
			wake();
		},
		wake,
	};
};
