// ###################
// A slider for seek and volume. WebKitGTK's <input type="range"> repaints slowly and stutters
// when its value is set while you drag it, so this is drawn from divs: drag with the mouse
// (one update per frame), click to jump, arrows / Home / End on the keyboard. Values are 0–1.
// set() is ignored while you drag, so the song's position never fights your hand
// ###################

// el: an empty element; it gets a track, a fill and a thumb (styled by --at, 0–1)
// onInput(v): while dragging (each frame); onCommit(v): when let go, clicked or a key pressed;
// onDraw(v): after the value is shown (the waveform redraws itself here)
export const makeSlider = (el, { onInput = () => {}, onCommit = () => {}, onDraw = () => {}, step = 0.05 } = {}) => {
	el.classList.add("slider");
	el.tabIndex = 0;
	el.setAttribute("role", "slider");
	el.setAttribute("aria-valuemin", "0");
	el.setAttribute("aria-valuemax", "100");
	el.innerHTML = `<span class="track"><span class="fill"></span></span><span class="thumb"></span>`;
	let value = 0;
	let dragging = false;
	let frame = 0;
	let pending = null;

	const draw = (v) => {
		el.style.setProperty("--at", String(v));
		el.setAttribute("aria-valuenow", String(Math.round(v * 100)));
		onDraw(v);
	};
	const at = (e) => {
		const box = el.getBoundingClientRect();
		return Math.min(1, Math.max(0, (e.clientX - box.left) / Math.max(1, box.width)));
	};
	const flush = () => {
		frame = 0;
		if (pending == null) return;
		value = pending;
		pending = null;
		draw(value);
		onInput(value);
	};

	el.addEventListener("pointerdown", (e) => {
		if (e.button !== 0) return;
		e.preventDefault();
		el.setPointerCapture(e.pointerId);
		el.focus();
		dragging = true;
		el.setAttribute("data-dragging", "");
		pending = at(e);
		flush();
	});
	el.addEventListener("pointermove", (e) => {
		if (!dragging) return;
		pending = at(e);
		frame ||= requestAnimationFrame(flush);
	});
	const end = (e) => {
		if (!dragging) return;
		if (frame) cancelAnimationFrame(frame), flush();
		dragging = false;
		el.removeAttribute("data-dragging");
		if (e.type === "pointerup") value = at(e), draw(value);
		onCommit(value);
	};
	el.addEventListener("pointerup", end);
	el.addEventListener("pointercancel", end);
	el.addEventListener("keydown", (e) => {
		const keys = { ArrowRight: step, ArrowUp: step, ArrowLeft: -step, ArrowDown: -step, PageUp: step * 4, PageDown: -step * 4 };
		let v = e.key in keys ? value + keys[e.key] : e.key === "Home" ? 0 : e.key === "End" ? 1 : null;
		if (v == null) return;
		e.preventDefault();
		e.stopPropagation();   // not the player's Ctrl+arrow keys
		value = Math.min(1, Math.max(0, v));
		draw(value);
		onInput(value);
		onCommit(value);
	});

	return {
		set: (v) => {
			if (dragging) return;
			value = Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0;
			draw(value);
		},
		dragging: () => dragging,
		value: () => value,
	};
};
