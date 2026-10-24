// ###################
// The About card (open / closed): the ⓘ button in the title bar opens it
// ###################
import { signal } from "arcmoon/reactive";

export const aboutOpen = signal(false);
export const openAbout = () => aboutOpen(true);
export const closeAbout = () => aboutOpen(false);
