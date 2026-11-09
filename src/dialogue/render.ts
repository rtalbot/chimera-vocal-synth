/**
 * Render a dialogue ahead of time: every participant gets an engine of its own (in a private
 * instance of the wasm module), all advance together in hops, each line starts on its speaker's
 * engine at its time, and each engine's telemetry is kept every hop so its head can replay it.
 * The voices are levelled and mixed into one track.
 *
 * Rendering ahead means any number of voices (no real-time budget), and the replay is exact.
 */
import { chimeraModule } from "../wasm/chimera.ts";
import { planPhrases, type Phrase } from "../wasm/engine.ts";
import type { BodyMix, Gesture } from "../wasm/types.ts";
import { writeGestures } from "../wasm/wire.ts";
import { g2p } from "../ui/g2p.ts";
import { wordSpans } from "../ui/words.ts";
import { PRESETS, defaultF0 } from "../ui/body.ts";
import type { Line } from "./script.ts";
import { TeleTrack, TELE_VERSION } from "../show/tele.ts";
import type { CastMember, Script } from "./script.ts";

export const SAMPLE_RATE = 48000;
/** samples per telemetry frame: 60 frames a second, as the live app */
export const HOP = 800;
const TELE_HEADER = 16;
const MAX_GESTURES = 4000;
/** silence before the first line, between lines, and after the last (s) */
const LEAD_IN = 0.5, GAP = 0.35, TAIL = 0.9;

type Fn = (...a: number[]) => number;
type Exports = { memory: WebAssembly.Memory } & Record<string, Fn>;

/** A word as said, on the dialogue's clock (s). */
export interface SaidWord {
  text: string;
  t0: number;
  t1: number;
}

/** One line as one participant said it. */
export interface SaidLine {
  t0: number;
  t1: number;
  text: string;
  words: SaidWord[];
}

export interface RenderedVoice {
  member: CastMember;
  lines: SaidLine[];
  /** the voice's telemetry, as a recorded track (its head replays it) */
  tele: TeleTrack;
}

export interface RenderedDialogue {
  sampleRate: number;
  hop: number;
  duration: number;
  /** the mix, as a WAV file */
  wav: ArrayBuffer;
  voices: RenderedVoice[];
}

/** A participant's engine and what it will say. */
interface Voice {
  member: CastMember;
  h: number;
  body: BodyMix;
  out: Float32Array;
  frames: Float32Array[];
  lines: SaidLine[];
  /** what to start, in time order */
  cues: { t: number; start: () => void }[];
}

export async function renderDialogue(script: Script, onProgress?: (done: number) => void, signal?: AbortSignal): Promise<RenderedDialogue> {
  if (!script.cast.length) throw new Error("the script has no cast");
  try {
    await g2p.load();
  } catch {
    // no dictionary: the letter-to-sound rules still speak
  }
  const x = new WebAssembly.Instance(chimeraModule(), {}).exports as unknown as Exports;

  const voices: Voice[] = script.cast.map((member) => {
    const h = x.engine_new(SAMPLE_RATE);
    const body = PRESETS[member.body];
    x.engine_set_body(h, body.female, body.child);
    return { member, h, body, out: new Float32Array(0), frames: [], lines: [], cues: [] };
  });

  // lay out the lines: each starts when the one before has ended; a shared line starts everyone at once
  let t = LEAD_IN;
  for (const step of script.steps) {
    if (step.kind === "pause") {
      t += step.seconds;
      continue;
    }
    const { line } = step;
    const start = t;
    let end = start;
    for (const k of line.speakers) {
      const v = voices[k];
      const said = planHuman(x, v, line, start);
      if (!said) continue;
      v.lines.push(said);
      end = Math.max(end, said.t1);
    }
    t = end + GAP;
  }
  const duration = t - GAP + TAIL;
  const total = Math.ceil((duration * SAMPLE_RATE) / HOP) * HOP;
  for (const v of voices) {
    v.out = new Float32Array(total);
    v.cues.sort((a, b) => a.t - b.t);
  }

  // render: all voices hop by hop, starting each line at the first hop at or after its time
  const hops = total / HOP;
  let lastYield = performance.now();
  for (let i = 0; i < hops; i++) {
    const now = (i * HOP) / SAMPLE_RATE;
    for (const v of voices) {
      while (v.cues.length && v.cues[0].t <= now + 1e-9) v.cues.shift()!.start();
      // the state at the start of the hop, as the recorded tracks expect (frame i is at sample i·hop)
      v.frames.push(humanFrame(x, v.h));
      const p = x.engine_render(v.h, HOP);
      v.out.set(new Float32Array(x.memory.buffer, p, HOP), i * HOP);
    }
    if (performance.now() - lastYield > 40) {
      if (signal?.aborted) throw new DOMException("cancelled", "AbortError");
      onProgress?.(i / hops);
      await yieldToPage();
      lastYield = performance.now();
    }
  }
  onProgress?.(1);

  const rendered: RenderedVoice[] = voices.map((v) => ({
    member: v.member,
    lines: v.lines,
    tele: new TeleTrack(teleFile(x, v)),
  }));
  for (const v of voices) x.engine_free(v.h);
  return { sampleRate: SAMPLE_RATE, hop: HOP, duration: total / SAMPLE_RATE, wav: wavFile(mix(voices.map((v) => v.out)), SAMPLE_RATE), voices: rendered };
}

/** Let the page draw and take input between stretches of rendering. A message is used, not a
 *  timer: timers in a background tab are slowed to one a second. */
function yieldToPage(): Promise<void> {
  return new Promise((r) => {
    const ch = new MessageChannel();
    ch.port1.onmessage = () => r();
    ch.port2.postMessage(0);
  });
}

/** Plan a human's line: English through the pronouncing dictionary; sung lines on one pitch. */
function planHuman(x: Exports, v: Voice, line: Line, start: number): SaidLine | null {
  const { text, sung } = line;
  let phrases: Phrase[] = g2p.toPhrases(text);
  if (!phrases.length) return null;
  const f0 = v.member.pitch ?? defaultF0(v.body);
  const shaped = phrases.map((p) => (sung ? { ...p, intonation: "flat" as const, pause: Math.min(p.pause, 0.15) } : p));
  const plan = planPhrases(shaped, { body: v.body, t0: 0, rate: sung ? 0.7 : 1, f0 });
  const gestures: Gesture[] = plan.gestures;
  v.cues.push({
    t: start,
    start: () => {
      const n = writeGestures(x.memory.buffer, x.engine_records(v.h), gestures, MAX_GESTURES);
      x.engine_schedule(v.h, n, 0, 0);
    },
  });
  const words = wordSpans(shaped, plan).map((w) => ({ text: w.text, t0: start + w.t0, t1: start + w.t1 }));
  return { t0: start, t1: start + plan.end, text, words };
}

/** One frame of a human's telemetry, in the recorded track's layout (see show/tele.ts). */
function humanFrame(x: Exports, h: number): Float32Array {
  const p = x.engine_telemetry(h);
  const head = new Float64Array(x.memory.buffer, p, TELE_HEADER);
  const [nArtic, nPh, nOr, nScope] = [head[11], head[12], head[13], head[14]];
  const body = new Float64Array(x.memory.buffer, p + 8 * TELE_HEADER, nArtic + 3 * (nPh + nOr) + 2 * nScope);
  const f = new Float32Array(11 + body.length);
  for (let k = 0; k < 11; k++) f[k] = head[k];
  f.set(body, 11);
  return f;
}

/** A human's frames as a telemetry file (the CLI's `--tele` layout), for TeleTrack. */
function teleFile(x: Exports, v: Voice): ArrayBuffer {
  const head = new Float64Array(x.memory.buffer, x.engine_telemetry(v.h), TELE_HEADER);
  const [nArtic, nPh, nOr, nScope] = [head[11], head[12], head[13], head[14]];
  const len = v.frames[0]?.length ?? 0;
  const buf = new ArrayBuffer(48 + 4 * len * v.frames.length);
  const dv = new DataView(buf);
  new Uint8Array(buf, 0, 4).set([0x43, 0x48, 0x54, 0x4c]); // "CHTL"
  [TELE_VERSION, SAMPLE_RATE, HOP, v.frames.length, len, nArtic, nPh, nOr, nScope].forEach((n, i) => dv.setUint32(4 + 4 * i, n, true));
  dv.setFloat32(40, v.body.female, true);
  dv.setFloat32(44, v.body.child, true);
  const data = new Float32Array(buf, 48);
  v.frames.forEach((f, i) => data.set(f, i * len));
  return buf;
}

/** Level each voice to the same peak, sum, and keep the sum under full scale. */
export function mix(tracks: Float32Array[]): Float32Array {
  const n = tracks[0]?.length ?? 0;
  const out = new Float32Array(n);
  for (const tr of tracks) {
    let peak = 0;
    for (let i = 0; i < n; i++) peak = Math.max(peak, Math.abs(tr[i]));
    if (!(peak > 1e-9)) continue;
    const g = 0.6 / peak;
    for (let i = 0; i < n; i++) out[i] += tr[i] * g;
  }
  let peak = 0;
  for (let i = 0; i < n; i++) peak = Math.max(peak, Math.abs(out[i]));
  if (peak > 0.95) {
    const g = 0.95 / peak;
    for (let i = 0; i < n; i++) out[i] *= g;
  }
  return out;
}

/** Mono 16-bit WAV. */
export function wavFile(samples: Float32Array, rate: number): ArrayBuffer {
  const buf = new ArrayBuffer(44 + 2 * samples.length);
  const dv = new DataView(buf);
  const str = (o: number, s: string) => [...s].forEach((c, i) => dv.setUint8(o + i, c.charCodeAt(0)));
  str(0, "RIFF");
  dv.setUint32(4, 36 + 2 * samples.length, true);
  str(8, "WAVE");
  str(12, "fmt ");
  dv.setUint32(16, 16, true);
  dv.setUint16(20, 1, true);
  dv.setUint16(22, 1, true);
  dv.setUint32(24, rate, true);
  dv.setUint32(28, rate * 2, true);
  dv.setUint16(32, 2, true);
  dv.setUint16(34, 16, true);
  str(36, "data");
  dv.setUint32(40, 2 * samples.length, true);
  for (let i = 0; i < samples.length; i++) dv.setInt16(44 + 2 * i, Math.max(-1, Math.min(1, samples[i])) * 32767, true);
  return buf;
}

/** The line a voice is saying at time t, and the word in it (−1 between words). */
export function lineAt(v: RenderedVoice, t: number): { line: SaidLine; word: number } | null {
  const line = v.lines.find((l) => t >= l.t0 - 0.05 && t <= l.t1 + 0.4);
  if (!line) return null;
  return { line, word: line.words.findIndex((w) => t >= w.t0 && t < w.t1 + 0.05) };
}
