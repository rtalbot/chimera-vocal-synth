/**
 * How a participant sounds, beyond which voice it is: settings written after the voice on its cast
 * line, named presets, and the ranges the Cast panel's sliders cover.
 *
 *   Theo = male gravelly slow pitch 95 rough 0.5
 *   └ voice ┘ └ presets ┘ └ settings (override the presets) ┘
 *
 * A number straight after the voice is its pitch (`Ava = female 230`), as before.
 */
import { DEFAULT_SOURCE } from "../wasm/engine.ts";
import type { BodyMix } from "../wasm/types.ts";

export type HumanBody = "male" | "female" | "child";
export const HUMAN_BODIES: readonly HumanBody[] = ["male", "female", "child"];
const BODY_MIX: Record<HumanBody, BodyMix> = { male: { female: 0, child: 0 }, female: { female: 1, child: 0 }, child: { female: 0, child: 1 } };

/** A setting: its word in a cast line, its label, its range and what it does. */
export interface Knob {
  key: string;
  label: string;
  min: number;
  max: number;
  step: number;
  hint: string;
  unit?: string;
  log?: boolean;
}

/** a human's settings (pitch's default depends on the body; the others are the engine's) */
export const HUMAN_KNOBS: readonly Knob[] = [
  { key: "pitch", label: "Pitch", min: 60, max: 500, step: 1, unit: "Hz", log: true, hint: "how high the voice sits" },
  { key: "range", label: "Expressive", min: 0, max: 0.5, step: 0.01, hint: "how far the pitch moves on stressed words: 0 is a monotone" },
  { key: "rate", label: "Speed", min: 0.5, max: 2, step: 0.05, unit: "×", hint: "speaking rate" },
  { key: "female", label: "Female", min: 0, max: 1, step: 0.05, hint: "the body between male and female: throat length and folds" },
  { key: "child", label: "Young", min: 0, max: 1, step: 0.05, hint: "the body toward a child's: smaller throat and folds" },
  { key: "breathy", label: "Breathy", min: 0, max: 1, step: 0.05, hint: "air through the folds: soft, whispery" },
  { key: "tense", label: "Pressed", min: 0, max: 1, step: 0.05, hint: "folds pressed together: edgy, urgent" },
  { key: "rough", label: "Rough", min: 0, max: 1, step: 0.05, hint: "irregular folds: gravelly, hoarse" },
  { key: "fry", label: "Fry", min: 0, max: 1, step: 0.05, hint: "creak at the bottom of the voice" },
  { key: "bright", label: "Bright", min: 0, max: 1, step: 0.05, hint: "more upper harmonics: brighter, thinner" },
  { key: "loud", label: "Loud", min: 4, max: 16, step: 0.5, unit: "cmH₂O", hint: "lung pressure: louder, harder" },
  { key: "larynx", label: "Larynx", min: -1.2, max: 1.2, step: 0.05, unit: "cm", hint: "voice box low (darker, bigger) or high (smaller, brighter)" },
  { key: "twang", label: "Twang", min: 0, max: 1, step: 0.05, hint: "a narrowed tube above the folds: a ringing, nasal-sounding twang" },
];

/** Named starting points: a few settings each (later words and explicit settings win). */
export const HUMAN_PRESETS: Record<string, { about: string; set: Record<string, number> }> = {
  gravelly: { about: "rough and low-lit", set: { rough: 0.45, fry: 0.25, bright: 0.15 } },
  whisper: { about: "mostly breath", set: { breathy: 0.75, loud: 6 } },
  breathy: { about: "soft, airy", set: { breathy: 0.45 } },
  creaky: { about: "vocal fry", set: { fry: 0.6 } },
  pressed: { about: "tight, urgent", set: { tense: 0.8, bright: 0.45 } },
  bright: { about: "brilliant, forward", set: { bright: 0.7, twang: 0.35 } },
  dark: { about: "low larynx, warm", set: { larynx: -0.8, bright: 0.1 } },
  booming: { about: "big and loud", set: { larynx: -0.6, loud: 13, range: 0.2 } },
  nasal: { about: "twangy", set: { twang: 0.8 } },
  old: { about: "frail, unsteady", set: { rough: 0.3, breathy: 0.3, rate: 0.85, fry: 0.2 } },
  monotone: { about: "flat, robotic", set: { range: 0.01 } },
  lively: { about: "big pitch swings", set: { range: 0.32, rate: 1.08 } },
  calm: { about: "even, unhurried", set: { range: 0.08, rate: 0.88, tense: 0.2 } },
  slow: { about: "slower", set: { rate: 0.8 } },
  fast: { about: "faster", set: { rate: 1.25 } },
};

export interface Tune {
  presets: string[];
  /** settings given explicitly */
  set: Record<string, number>;
}

export const NO_TUNE: Tune = { presets: [], set: {} };

/** Read the words after a cast line's voice. Unknown words come back as errors. */
export function parseTune(tokens: string[]): { tune: Tune; errors: string[] } {
  const knobs = HUMAN_KNOBS;
  const presets = HUMAN_PRESETS;
  const tune: Tune = { presets: [], set: {} };
  const errors: string[] = [];
  for (let i = 0; i < tokens.length; i++) {
    const w = tokens[i].toLowerCase();
    const num = (s: string | undefined) => (s !== undefined && /^-?[\d.]+$/.test(s) ? Number(s) : NaN);
    if (i === 0 && Number.isFinite(num(w))) {
      tune.set.pitch = num(w);
      continue;
    }
    const k = knobs.find((k) => k.key.toLowerCase() === w);
    // a word that is both (breathy, bright) is a setting when a number follows, else the preset
    if (w in presets && !(k && Number.isFinite(num(tokens[i + 1])))) {
      tune.presets.push(w);
      continue;
    }
    if (k) {
      const v = num(tokens[i + 1]);
      if (!Number.isFinite(v)) errors.push(`"${tokens[i]}" needs a number after it (${k.min}…${k.max})`);
      else {
        tune.set[k.key] = Math.min(k.max, Math.max(k.min, v));
        i++;
      }
      continue;
    }
    const known = [...Object.keys(presets), ...knobs.map((k) => k.key)].join(", ");
    errors.push(`"${tokens[i]}" isn't a setting or a preset (try: ${known})`);
  }
  return { tune, errors };
}

/** The words for a cast line (presets, then settings), as parseTune reads them. */
export function formatTune(t: Tune): string {
  const nums = Object.entries(t.set).map(([k, v]) => `${k} ${+v.toFixed(k === "pitch" ? 0 : 2)}`);
  return [...t.presets, ...nums].join(" ");
}

/** The settings in force: presets in order, then the explicit ones. */
export function effective(t: Tune): Record<string, number> {
  const presets = HUMAN_PRESETS;
  const out: Record<string, number> = {};
  for (const p of t.presets) Object.assign(out, presets[p]?.set);
  return Object.assign(out, t.set);
}

/** A human's voice, everything resolved: what the renderer sets on the engine. */
export interface HumanVoiceSettings {
  body: BodyMix;
  f0: number;
  rate: number;
  accent: number;
  source: { breathiness: number; tension: number; roughness: number; fry: number; brightness: number; pressure: number; vibratoDepth: number; drift: number; voiced: number };
  artic: { larynx: number; epilarynx: number };
}

const defaultF0 = (b: BodyMix) => {
  // as src/ui/body.ts: log-blend of 110, 205 and 260 Hz by the body's weights
  const f = b.female, c = b.child;
  const w = [(1 - c) * (1 - f), (1 - c) * f, c];
  return Math.exp(w[0] * Math.log(110) + w[1] * Math.log(205) + w[2] * Math.log(260));
};

export function humanSettings(base: HumanBody, t: Tune): HumanVoiceSettings {
  const e = effective(t);
  const body = { female: e.female ?? BODY_MIX[base].female, child: e.child ?? BODY_MIX[base].child };
  const s = DEFAULT_SOURCE;
  return {
    body,
    f0: e.pitch ?? defaultF0(body),
    rate: e.rate ?? 1,
    accent: e.range ?? 0.14,
    source: { breathiness: e.breathy ?? s.breathiness, tension: e.tense ?? s.tension, roughness: e.rough ?? s.roughness, fry: e.fry ?? s.fry, brightness: e.bright ?? s.brightness, pressure: e.loud ?? s.pressure, vibratoDepth: s.vibratoDepth, drift: s.drift, voiced: 1 },
    artic: { larynx: e.larynx ?? 0, epilarynx: e.twang ?? 0 },
  };
}

/** The value a slider shows for a knob: the setting in force, or the voice's default. */
export function knobValue(k: Knob, t: Tune, base: HumanBody): number {
  const e = effective(t);
  if (e[k.key] !== undefined) return e[k.key];
  const h = humanSettings(base, t);
  const d: Record<string, number> = { pitch: h.f0, range: h.accent, rate: h.rate, female: h.body.female, child: h.body.child, breathy: h.source.breathiness, tense: h.source.tension, rough: h.source.roughness, fry: h.source.fry, bright: h.source.brightness, loud: h.source.pressure, larynx: h.artic.larynx, twang: h.artic.epilarynx };
  return d[k.key];
}

/**
 * Settings that tell apart characters given the same voice: the first of a body sounds as it is,
 * the next ones a little lower or higher, rougher or breathier, quicker or calmer.
 */
const VARIANTS: readonly Record<string, number>[] = [
  {},
  { pitchBy: 0.86, rough: 0.25, range: 0.1, rate: 0.92 },
  { pitchBy: 1.12, breathy: 0.35, range: 0.22, rate: 1.06 },
  { pitchBy: 0.94, bright: 0.55, tense: 0.5 },
  { pitchBy: 1.05, fry: 0.35, larynx: -0.5, rate: 0.95 },
];
export function variant(base: HumanBody, nth: number): Tune {
  const v = VARIANTS[nth % VARIANTS.length];
  const set: Record<string, number> = {};
  for (const [k, x] of Object.entries(v)) {
    if (k === "pitchBy") set.pitch = Math.round(defaultF0(BODY_MIX[base]) * x);
    else set[k] = x;
  }
  return { presets: [], set };
}
