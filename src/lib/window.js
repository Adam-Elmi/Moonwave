// ###################
// The window (Tauri): minimize, maximize / restore, close. In a plain browser these do nothing
// ###################

const win = globalThis.window?.__TAURI__?.window.getCurrentWindow() ?? null;

export const inWindow = win !== null;
export const minimize = () => win?.minimize();
export const toggleMaximize = () => win?.toggleMaximize();
export const close = () => win?.close();
