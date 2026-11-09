/**
 * A dialogue script: who is in it, and who says what, in order.
 *
 *   # the cast: a name, "=", a voice (male, female or child), then how it sounds (see tune.ts)
 *   Ava = female
 *   Theo = male gravelly pitch 95 rate 0.9   ← presets and settings: see tune.ts
 *   Kit = child
 *
 *   # the lines: one after another
 *   Ava: Hello there. Who are you?
 *   Kit: We have waited for you.
 *   Ava + Theo: Welcome!          ← together: both start at once
 *   Ava (a little sad): I know.   ← how it's said: emotions, see emotion.ts
 *   Kit sings: We cannot stop.    ← "sings": on one pitch
 *   (pause 1.5)                   ← silence, in seconds
 *
 * Blank lines and lines starting with # are ignored. Names are matched without regard to case.
 */

import { HUMAN_BODIES, NO_TUNE, formatTune, parseTune, variant, type HumanBody, type Tune } from "./tune.ts";
import { EMOTIONS, formatMood, parseMood, type Mood } from "./emotion.ts";
export { HUMAN_BODIES, type HumanBody };

/** A participant: a body, and how it's tuned. */
export interface CastMember {
  name: string;
  body: HumanBody;
  tune: Tune;
}

export interface Line {
  /** indices into the cast: more than one speak together */
  speakers: number[];
  text: string;
  sung: boolean;
  /** how it's said: an emotion or two, and how strongly */
  mood?: Mood;
}

export type Step = { kind: "line"; line: Line } | { kind: "pause"; seconds: number };

export interface Script {
  cast: CastMember[];
  steps: Step[];
}

export interface ScriptError {
  line: number;
  message: string;
}

/** Parse a script. */
export function parseScript(text: string): { script: Script; errors: ScriptError[] } {
  const cast: CastMember[] = [];
  const steps: Step[] = [];
  const errors: ScriptError[] = [];
  const find = (name: string) => cast.findIndex((c) => c.name.toLowerCase() === name.trim().toLowerCase());

  text.split(/\r?\n/).forEach((raw, i) => {
    const n = i + 1;
    const s = raw.trim();
    if (!s || s.startsWith("#")) return;

    const pause = /^\(\s*pause(?:\s+([\d.]+))?\s*s?\s*\)$/i.exec(s);
    if (pause) {
      steps.push({ kind: "pause", seconds: Math.min(10, Number(pause[1] ?? 1) || 1) });
      return;
    }

    const member = /^([^:=]+?)\s*=\s*([\w-]+)((?:\s+[\w.-]+)*)\s*$/.exec(s);
    if (member) {
      const name = member[1].trim();
      const voice = member[2].toLowerCase();
      const words = member[3].trim() ? member[3].trim().split(/\s+/) : [];
      if (find(name) >= 0) return errors.push({ line: n, message: `${name} is already in the cast` });
      if (!(HUMAN_BODIES as readonly string[]).includes(voice)) return errors.push({ line: n, message: `"${member[2]}" isn't a voice: use male, female or child` });
      const { tune, errors: bad } = parseTune(words);
      for (const m of bad) errors.push({ line: n, message: `${name}: ${m}` });
      cast.push({ name, body: voice as HumanBody, tune });
      return;
    }

    const said = /^([^:]+?)\s*:\s*(.*)$/.exec(s);
    if (said) {
      // "Ava (a little sad) sings: …": the mood in parentheses, "sings" at the end
      let head = said[1];
      const moodWords = [...head.matchAll(/\(([^)]*)\)/g)].map((m) => m[1]);
      head = head.replace(/\([^)]*\)/g, " ").replace(/\s+/g, " ").trim();
      const sung = /\s+sings?$/i.test(head);
      head = head.replace(/\s+sings?$/i, "");
      // a stage direction at the start of the words, as screenplays write it: "(sobbing) Where…"
      let words = said[2].trim();
      const lead = /^\(([^)]*)\)\s*/.exec(words);
      if (lead) {
        words = words.slice(lead[0].length);
        moodWords.push(lead[1]);
      }
      let mood: Mood | null = null;
      if (moodWords.length) {
        const parsed = parseMood(moodWords.join(", "));
        mood = parsed.mood;
        // in the speaker's parentheses, a word that isn't an emotion is a mistake; in the words, a stage direction
        const inHead = parseMood([...said[1].matchAll(/\(([^)]*)\)/g)].map((m) => m[1]).join(", "));
        if (inHead.unknown.length) errors.push({ line: n, message: `"${inHead.unknown.join(", ")}" isn't an emotion (try: ${Object.keys(EMOTIONS).join(", ")})` });
      }
      const names = head.split(/\s*(?:\+|&|\band\b)\s*/i).filter(Boolean);
      const speakers: number[] = [];
      for (const name of names) {
        const k = find(name);
        if (k < 0) errors.push({ line: n, message: `${name.trim()} isn't in the cast (add "${name.trim()} = female", say)` });
        else if (!speakers.includes(k)) speakers.push(k);
      }
      if (speakers.length && words) steps.push({ kind: "line", line: { speakers, text: words, sung, ...(mood ? { mood } : {}) } });
      return;
    }

    errors.push({ line: n, message: `Not a cast line ("Name = voice"), a line ("Name: words") or a pause` });
  });

  if (cast.length > MAX_CAST) errors.push({ line: 0, message: `At most ${MAX_CAST} in the cast (there are ${cast.length})` });
  const words = scriptWords({ cast, steps });
  if (words > MAX_WORDS) errors.push({ line: 0, message: `At most ${MAX_WORDS} words of dialogue (there are ${words}): shorten it or split it in two` });
  if (!cast.length && !errors.length) errors.push({ line: 0, message: "Add a cast: one line per participant, like \"Ava = female\"" });
  return { script: { cast, steps }, errors };
}

/** The most participants on one stage, and the most words of dialogue in one render. */
export const MAX_CAST = 8;
export const MAX_WORDS = 600;

const VOICE_CYCLE: readonly HumanBody[] = ["female", "male"];

/** Words in a line, as the limit counts them. */
const wordCount = (s: string) => s.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w)).length;

export interface Imported {
  /** the dialogue as a script for this page: a cast (voices guessed, change them in the cast) and lines */
  text: string;
  characters: string[];
  lines: number;
  words: number;
  /** what was left out to keep within the limits */
  dropped: { lines: number; characters: string[] };
}

/**
 * A screenplay (or any "NAME: words" text) → a script. Screenplays put a character's name in
 * capitals on its own line, their words on the lines under it until a blank line; extensions like
 * (V.O.) and (CONT'D) are dropped, as are parentheticals, scene headings, transitions and action.
 * Fountain's forced names (@McCLANE) are kept. Characters beyond MAX_CAST, and lines beyond
 * MAX_WORDS, are left out (and counted).
 */
export function importScreenplay(text: string, voices: readonly HumanBody[] = VOICE_CYCLE): Imported {
  const said: { name: string; words: string; mood?: string }[] = [];
  const rows = text.replace(/\r\n?/g, "\n").split("\n");
  const cue = (raw: string): string | null => {
    const s = raw.trim();
    if (s.startsWith("@")) return clean(s.slice(1));
    if (!s || s.length > 40 || /[a-z]/.test(s.replace(/\(.*?\)/g, ""))) return null;
    if (/^(INT|EXT|EST|INT\.?\/EXT|I\/E)[.\s]/.test(s) || /(TO:|FADE IN:?|FADE OUT\.?|THE END)$/.test(s) || /^[>\-=#!\[]/.test(s)) return null;
    if (!/[A-Z]/.test(s)) return null;
    return clean(s);
  };
  const clean = (s: string) =>
    s
      .replace(/\(.*?\)/g, "")
      .replace(/\^$/, "")
      .replace(/\s+/g, " ")
      .trim()
      .toLowerCase()
      .replace(/(^|[\s'-])\p{L}/gu, (m) => m.toUpperCase());

  // "NAME: words" lines, if the text is written that way
  const colon = rows.filter((r) => /^\s*[A-Za-z][\w .'-]{0,30}:\s*\S/.test(r) && !/^\s*(INT|EXT)\./.test(r));
  if (colon.length >= 2 && colon.length >= rows.filter((r) => cue(r)).length) {
    for (const r of colon) {
      const m = /^\s*([^:]+):\s*(.*)$/.exec(r)!;
      const paren = /\(([^)]*)\)/.exec(m[1]);
      const mood = paren ? parseMood(paren[1]).mood : null;
      said.push({ name: clean(m[1]), words: m[2].trim(), ...(mood ? { mood: formatMood(mood) } : {}) });
    }
  } else {
    for (let i = 0; i < rows.length; i++) {
      const name = cue(rows[i]);
      if (!name || i + 1 >= rows.length || !rows[i + 1].trim()) continue;
      // the speech, split where a direction gives a new mood: "(sobbing)" … "(suddenly furious)"
      let words: string[] = [];
      let mood: string | undefined;
      const flush = () => {
        const w = words.join(" ").replace(/\s+/g, " ").trim();
        if (w) said.push({ name, words: w, ...(mood ? { mood } : {}) });
        words = [];
      };
      let j = i + 1;
      for (; j < rows.length && rows[j].trim(); j++) {
        const s = rows[j].trim();
        const direction = /^\((.*)\)$/.exec(s);
        const found = direction ? parseMood(direction[1]).mood : null;
        if (direction) {
          // (beat), (to Dev): not a mood, skipped; a mood: from here on
          if (found) {
            flush();
            mood = formatMood(found);
          }
          continue;
        }
        words.push(s.replace(/\(.*?\)/g, "").trim());
      }
      flush();
      i = j;
    }
  }

  // keep the first MAX_CAST characters to speak and the lines up to MAX_WORDS
  const characters: string[] = [];
  const kept: typeof said = [];
  const droppedNames = new Set<string>();
  let words = 0, droppedLines = 0;
  for (const l of said) {
    const n = wordCount(l.words);
    const known = characters.includes(l.name);
    if (!known && characters.length >= MAX_CAST) {
      droppedNames.add(l.name);
      droppedLines++;
      continue;
    }
    if (words + n > MAX_WORDS) {
      droppedLines++;
      continue;
    }
    if (!known) characters.push(l.name);
    kept.push(l);
    words += n;
  }

  // characters given the same voice are told apart (a little lower, rougher, breathier…)
  const seen: Record<string, number> = {};
  const castText = characters
    .map((c, i) => {
      const v = voices[i % voices.length];
      const nth = (seen[v] = (seen[v] ?? -1) + 1);
      const t = formatTune(variant(v, nth));
      return `${c} = ${v}${t ? " " + t : ""}`;
    })
    .join("\n");
  const out = `# Cast: change who reads each part (male, female or child)\n${castText}\n\n${kept.map((l) => `${l.name}${l.mood ? ` (${l.mood})` : ""}: ${l.words}`).join("\n")}\n`;
  return { text: out, characters, lines: kept.length, words, dropped: { lines: droppedLines, characters: [...droppedNames] } };
}

/** The script with one cast member's line rewritten in place: its voice and its tuning. */
export function setMember(text: string, name: string, voice: string, tune: Tune = NO_TUNE): string {
  const lines = text.split("\n");
  const i = lines.findIndex((l) => {
    const m = /^\s*([^:=#]+?)\s*=\s*[\w-]+/.exec(l);
    return m && m[1].trim().toLowerCase() === name.toLowerCase();
  });
  if (i < 0) return text;
  const t = formatTune(tune);
  lines[i] = `${name} = ${voice}${t ? " " + t : ""}`;
  return lines.join("\n");
}

/** A new body for a part, keeping its tuning. */
export function setVoice(text: string, member: CastMember, body: HumanBody): string {
  return setMember(text, member.name, body, member.tune);
}

/** Words of dialogue in a script, as the limit counts them. */
export function scriptWords(script: Script): number {
  return script.steps.reduce((n, s) => n + (s.kind === "line" ? wordCount(s.line.text) : 0), 0);
}

export const EXAMPLE_SCRIPT = `# The cast: a name = male, female or child, then how they sound
Ava = female lively
Theo = male gravelly pitch 100
Kit = child bright

# Then who says what, in order, and how
Ava (nervous): Hello? Is somebody there?
Kit: Over here! I found something.
Theo (terrified): Don't touch it! What is it?
Kit (excited): It's humming. Listen!
(pause 0.5)
Ava (calm): It's only the wind in the pipes.
Ava + Theo + Kit (joyful): Then let us sing!
Kit sings: Together at last.
`;
