/**
 * Aliens singing from the keys: with "Keys play: Words" chosen in the Bestiary, each key press sings
 * the next word of the Speak box's text, in the alien's language, on the key's pitch.
 */
import { alienSpeak } from "../wasm/engine.ts";
import { engine } from "../audio/AudioEngine.ts";

/** The Bestiary's `keyCall` value for "the keys sing the words". */
export const SING_WORDS = "__words";

let words: string[] = [];
let next = 0;
const listeners = new Set<(i: number) => void>();

/** The text to sing from (the Speak box's); starts again from its first word when it changes. */
export function setSungText(text: string): void {
  const w = text.split(/\s+/).map((s) => s.replace(/[^\p{L}\p{N}']/gu, "")).filter(Boolean);
  if (w.join(" ") !== words.join(" ")) {
    words = w;
    next = 0;
  }
}

/** Sing the next word on `hz` (wrapping at the end). Returns the index sung, or −1. */
export function singNextWord(creature: string, hz: number | undefined): number {
  if (!words.length) return -1;
  const i = next % words.length;
  const u = alienSpeak(creature, words[i], { pitch: hz, sung: true });
  if (!u) return -1;
  engine.speakAlien(u.flat);
  next = i + 1;
  for (const f of listeners) f(i);
  return i;
}

/** Hear which word was sung (for highlighting); returns the unsubscribe. */
export function onSungWord(f: (i: number) => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}
