// ###################
// npm run watch: build once, then again whenever something in pages/ or src/ changes
// (arcm build has no watch mode); tauri dev runs this and shows dist/
// ###################

import { watch } from "node:fs";
import { spawn } from "node:child_process";

const FOLDERS = ["pages", "src"];
let running = false;
let again = false;
let timer = null;

const build = () => {
	if (running) return (again = true);
	running = true;
	const child = spawn("npx", ["arcm", "build", "pages"], { stdio: "inherit" });
	child.on("exit", () => {
		running = false;
		if (again) {
			again = false;
			build();
		}
	});
};

// many saves at once (an editor writing several files) give one build
const soon = () => {
	clearTimeout(timer);
	timer = setTimeout(build, 100);
};

for (const folder of FOLDERS) watch(folder, { recursive: true }, soon);
build();
console.log(`watching ${FOLDERS.join(", ")}; Ctrl+C to stop`);
