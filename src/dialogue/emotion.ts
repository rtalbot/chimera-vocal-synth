/**
 * How a line is said: an emotion (or two) and how strongly, written in parentheses after the
 * speaker (`Ava (distraught): …`, `Theo (a little angry, whispering): …`) or taken from a
 * screenplay's parenthetical under the character's name. Each emotion is a set of changes to the
 * speaker's voice for that line: pitch, its range, speed, breath, pressure, roughness, tremor.
 */
import type { HumanVoiceSettings } from "./tune.ts";

/** Changes at full strength: `…By` multiply, the rest add. */
export interface EmotionShift {
  pitchBy?: number;
  rangeBy?: number;
  range?: number;
  rateBy?: number;
  breathy?: number;
  tense?: number;
  rough?: number;
  fry?: number;
  bright?: number;
  loud?: number;
  /** pitch tremor (cents) */
  tremor?: number;
  /** slow random pitch wander (cents) */
  drift?: number;
  /** above one half: no voicing at all (a true whisper) */
  unvoiced?: number;
}

export const EMOTIONS: Record<string, { about: string; shift: EmotionShift }> = {
  joyful: { about: "high, bright, bouncing pitch", shift: { pitchBy: 1.15, range: 0.18, rateBy: 1.1, bright: 0.25, loud: 2, breathy: 0.05 } },
  excited: { about: "high, fast, loud", shift: { pitchBy: 1.2, range: 0.2, rateBy: 1.25, loud: 3, bright: 0.3, tense: 0.2 } },
  sad: { about: "low, slow, flat, soft", shift: { pitchBy: 0.9, rangeBy: 0.5, rateBy: 0.82, breathy: 0.25, loud: -2, bright: -0.15 } },
  distraught: { about: "unsteady, breathy, breaking", shift: { pitchBy: 1.12, range: 0.15, rateBy: 0.9, breathy: 0.35, rough: 0.3, tense: 0.2, loud: 1, tremor: 40, drift: 15 } },
  crying: { about: "shaking, breathy, cracking", shift: { pitchBy: 1.15, range: 0.1, rateBy: 0.8, breathy: 0.45, rough: 0.35, fry: 0.12, tremor: 60, drift: 25 } },
  angry: { about: "pressed, loud, clipped", shift: { pitchBy: 1.05, range: 0.1, rateBy: 1.1, tense: 0.45, loud: 4, rough: 0.15, bright: 0.3 } },
  furious: { about: "shouted, hard, rough", shift: { pitchBy: 1.15, range: 0.2, rateBy: 1.15, tense: 0.6, loud: 6, rough: 0.3, bright: 0.4 } },
  afraid: { about: "high, quick, breathy, shaky", shift: { pitchBy: 1.15, range: 0.05, rateBy: 1.15, breathy: 0.25, tense: 0.25, tremor: 30, drift: 10 } },
  terrified: { about: "very high, fast, trembling", shift: { pitchBy: 1.3, range: 0.1, rateBy: 1.3, breathy: 0.35, tense: 0.35, loud: 2, tremor: 55, drift: 20 } },
  nervous: { about: "a little high, hurried, unsteady", shift: { pitchBy: 1.08, rateBy: 1.12, breathy: 0.15, tremor: 40, drift: 20 } },
  calm: { about: "even, unhurried", shift: { pitchBy: 0.96, rangeBy: 0.7, rateBy: 0.9, tense: -0.2, loud: -1 } },
  tired: { about: "low, slow, flat, creaky", shift: { pitchBy: 0.9, rangeBy: 0.5, rateBy: 0.8, breathy: 0.25, fry: 0.2, loud: -2 } },
  bored: { about: "flat and slow", shift: { pitchBy: 0.93, rangeBy: 0.3, rateBy: 0.88, fry: 0.12 } },
  whispering: { about: "breath only, quiet", shift: { unvoiced: 1, breathy: 0.8, loud: -3, tense: -0.3 } },
  shouting: { about: "loud, high, pressed", shift: { pitchBy: 1.2, range: 0.1, loud: 7, tense: 0.4, bright: 0.35 } },
  sarcastic: { about: "exaggerated pitch, drawn out", shift: { range: 0.25, rateBy: 0.88 } },
  tender: { about: "soft, warm, gentle", shift: { pitchBy: 1.02, range: 0.05, rateBy: 0.9, breathy: 0.25, loud: -2, tense: -0.2 } },
};

/** Words that mean one of the emotions above (adverbs ending -ly are tried without it too). */
const SYNONYMS: Record<string, string> = {
  happy: "joyful", glad: "joyful", cheerful: "joyful", overjoyed: "joyful", elated: "joyful", ecstatic: "joyful", delighted: "joyful", laughing: "joyful", gleeful: "joyful", joy: "joyful",
  thrilled: "excited", eager: "excited", enthusiastic: "excited",
  unhappy: "sad", glum: "sad", miserable: "sad", sorrowful: "sad", melancholy: "sad", mournful: "sad", grieving: "sad", heartbroken: "sad",
  upset: "distraught", anguished: "distraught", devastated: "distraught", hysterical: "distraught", frantic: "distraught", desperate: "distraught", pleading: "distraught", broken: "distraught",
  sobbing: "crying", weeping: "crying", tearful: "crying", tears: "crying", "in tears": "crying",
  mad: "angry", annoyed: "angry", irritated: "angry", cross: "angry", bitter: "angry", frustrated: "angry", stern: "angry",
  enraged: "furious", livid: "furious", raging: "furious", seething: "furious",
  scared: "afraid", frightened: "afraid", fearful: "afraid", worried: "nervous", anxious: "nervous", hesitant: "nervous", uneasy: "nervous", stammering: "nervous",
  panicked: "terrified", panicking: "terrified", horrified: "terrified", petrified: "terrified",
  quiet: "calm", relaxed: "calm", soothing: "calm", serene: "calm", matter: "calm",
  exhausted: "tired", sleepy: "tired", weary: "tired", drowsy: "tired",
  flat: "bored", deadpan: "bored", monotone: "bored", disinterested: "bored", uninterested: "bored",
  whisper: "whispering", whispers: "whispering", whispered: "whispering", hushed: "whispering", "under breath": "whispering", "under her breath": "whispering", "under his breath": "whispering",
  shout: "shouting", shouts: "shouting", yelling: "shouting", yells: "shouting", screaming: "shouting", screams: "shouting", loud: "shouting", bellowing: "shouting",
  dry: "sarcastic", mocking: "sarcastic", wry: "sarcastic", ironic: "sarcastic", teasing: "sarcastic",
  soft: "tender", gentle: "tender", loving: "tender", warm: "tender", affectionate: "tender", kind: "tender", softly: "tender", gently: "tender",
};

const STRONG = /^(very|really|so|extremely|completely|utterly|totally|deeply|truly|incredibly)\s+/;
const WEAK = /^(a little|a bit|slightly|somewhat|mildly|faintly|rather|kind of|sort of)\s+/;
/** strength of an emotion without a word or number for it */
export const DEFAULT_STRENGTH = 0.75;

export interface Mood {
  /** emotion → strength (0–1.5) */
  parts: { emotion: string; strength: number }[];
}

/** One word or phrase → an emotion's name, or null. */
export function emotionOf(word: string): string | null {
  const w = word.toLowerCase().trim().replace(/[^a-z' ]/g, "").replace(/\s+/g, " ");
  if (!w) return null;
  if (w in EMOTIONS) return w;
  if (w in SYNONYMS) return SYNONYMS[w];
  // adverbs: angrily → angry, sadly → sad, excitedly → excited
  const stems = [w.replace(/ily$/, "y"), w.replace(/ly$/, ""), w.replace(/ingly$/, "ing"), w.replace(/edly$/, "ed")];
  for (const s of stems) {
    if (s in EMOTIONS) return s;
    if (s in SYNONYMS) return SYNONYMS[s];
  }
  return null;
}

/**
 * The words in a parenthetical → a mood: "a little sad", "very angry, whispering", "joyful 0.4".
 * `unknown` lists the parts that aren't emotions (a screenplay's "(to Theo)" or "(beat)").
 */
export function parseMood(text: string): { mood: Mood | null; unknown: string[] } {
  const parts: Mood["parts"] = [];
  const unknown: string[] = [];
  for (const raw of text.split(/\s*(?:,|;|\band\b|&|\/)\s*/i)) {
    let s = raw.toLowerCase().trim().replace(/[.!?]+$/, "");
    if (!s) continue;
    let strength = DEFAULT_STRENGTH;
    const num = /\s+(\d*\.?\d+)$/.exec(s);
    if (num) {
      strength = Math.min(1.5, Math.max(0, Number(num[1])));
      s = s.slice(0, num.index);
    } else if (STRONG.test(s)) {
      strength = 1;
      s = s.replace(STRONG, "");
    } else if (WEAK.test(s)) {
      strength = 0.4;
      s = s.replace(WEAK, "");
    }
    // a phrase may be several words: try it whole, then each word ("smiling, almost laughing")
    let e = emotionOf(s);
    if (!e) for (const w of s.split(" ").reverse()) if ((e = emotionOf(w))) break;
    if (e) {
      const prev = parts.find((p) => p.emotion === e);
      if (prev) prev.strength = Math.max(prev.strength, strength);
      else parts.push({ emotion: e, strength });
    } else unknown.push(raw.trim());
  }
  return { mood: parts.length ? { parts } : null, unknown };
}

/** The mood written back as words, as parseMood reads them. */
export function formatMood(m: Mood): string {
  return m.parts.map((p) => (Math.abs(p.strength - DEFAULT_STRENGTH) < 1e-6 ? p.emotion : p.strength >= 1 && p.strength < 1.0001 ? `very ${p.emotion}` : `${p.emotion} ${+p.strength.toFixed(2)}`)).join(", ");
}

/** A human's voice for one line: its own settings, shifted by the line's mood. */
export function withMood(s: HumanVoiceSettings, m: Mood | null | undefined): HumanVoiceSettings {
  if (!m) return s;
  const k = moodShift(m);
  const c01 = (x: number) => Math.min(1, Math.max(0, x));
  const src = s.source;
  return {
    ...s,
    f0: s.f0 * k.pitchBy,
    rate: s.rate * k.rateBy,
    accent: Math.max(0, s.accent * k.rangeBy + k.range),
    source: {
      breathiness: c01(src.breathiness + k.breathy),
      tension: c01(src.tension + k.tense),
      roughness: c01(src.roughness + k.rough),
      fry: c01(src.fry + k.fry),
      brightness: c01(src.brightness + k.bright),
      pressure: Math.min(20, Math.max(3, src.pressure + k.loud)),
      vibratoDepth: Math.max(0, src.vibratoDepth + k.tremor),
      drift: Math.max(0, src.drift + k.drift),
      voiced: k.unvoiced >= 0.5 ? 0 : src.voiced,
    },
  };
}

/** A mood's combined shift: each emotion scaled by its strength (multipliers combine by product). */
export function moodShift(m: Mood | null | undefined): Required<EmotionShift> {
  const out: Required<EmotionShift> = { pitchBy: 1, rangeBy: 1, range: 0, rateBy: 1, breathy: 0, tense: 0, rough: 0, fry: 0, bright: 0, loud: 0, tremor: 0, drift: 0, unvoiced: 0 };
  for (const p of m?.parts ?? []) {
    const s = EMOTIONS[p.emotion]?.shift;
    if (!s) continue;
    const k = p.strength;
    for (const key of ["pitchBy", "rangeBy", "rateBy"] as const) if (s[key] !== undefined) out[key] *= 1 + (s[key]! - 1) * k;
    for (const key of ["range", "breathy", "tense", "rough", "fry", "bright", "loud", "tremor", "drift", "unvoiced"] as const) if (s[key] !== undefined) out[key] += s[key]! * k;
  }
  return out;
}
