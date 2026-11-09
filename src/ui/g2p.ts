/** The page's one English pronouncer (the engine keeps one dictionary): shared by speech and dialogue. */
import { EnglishG2P } from "../wasm/engine.ts";

export const g2p = new EnglishG2P();
