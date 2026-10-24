// ###################
// The song menu (right-click a song): { song, x, y, list, position } while open; list and position
// only when the song was right-clicked on one of your lists (for Remove Song from List)
// ###################
import { signal } from "arcmoon/reactive";

export const menu = signal(null);
export const openMenu = (song, x, y, inList = null) => menu({ song, x, y, list: inList?.id ?? null, position: inList?.position ?? -1 });
export const closeMenu = () => menu(null);
