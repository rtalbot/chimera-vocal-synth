/**
 * Dialogues: the script parser, and rendering several voices ahead of time (one engine each),
 * with telemetry for every participant's head and one levelled mix.
 */
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { initEngine } from "./harness.ts";

await initEngine();
const { parseScript, importScreenplay, setVoice, MAX_CAST, MAX_WORDS } = await import("../src/dialogue/script.ts");
const { renderDialogue, lineAt, HOP, SAMPLE_RATE } = await import("../src/dialogue/render.ts");
const { g2p } = await import("../src/ui/g2p.ts");
g2p.setDictionaryText(gunzipSync(readFileSync(new URL("../public/data/cmudict.txt.gz", import.meta.url))).toString());

describe("dialogue script", () => {
  test("cast, lines, together, sung and pauses", () => {
    const { script, errors } = parseScript(
      `# cast
Ava = female
theo = male 100
Kit = child

Ava: Hello there.
Ava + Theo: Welcome!
Kit sings: We sing.
(pause 1.5)
Theo & kit: Bye.`,
    );
    expect(errors).toEqual([]);
    expect(script.cast.map((c) => [c.name, c.body])).toEqual([["Ava", "female"], ["theo", "male"], ["Kit", "child"]]);
    expect(script.cast[1].pitch).toBe(100);
    expect(script.steps).toEqual([
      { kind: "line", line: { speakers: [0], text: "Hello there.", sung: false } },
      { kind: "line", line: { speakers: [0, 1], text: "Welcome!", sung: false } },
      { kind: "line", line: { speakers: [2], text: "We sing.", sung: true } },
      { kind: "pause", seconds: 1.5 },
      { kind: "line", line: { speakers: [1, 2], text: "Bye.", sung: false } },
    ]);
  });

  test("unknown voices and speakers are reported with their line", () => {
    const { errors } = parseScript("Ava = robot\nBob: hi");
    expect(errors.map((e) => e.line)).toEqual([1, 2]);
  });
});

// an original scrap of screenplay, in the usual layout
const SCREENPLAY = `FADE IN:

INT. OBSERVATORY - NIGHT

Rain on the dome. MARA (40s) checks a readout.

                    MARA
          It's back. The same signal.

                    DEV (O.S.)
          (from the doorway)
          Which one? We've had three
          this week.

                    MARA (CONT'D)
          The one that hums.

A long beat. Dev crosses to the console.

                    DEV
          Then let's answer it.

CUT TO:

EXT. RIDGE - CONTINUOUS
`;

describe("screenplay import", () => {
  test("names in capitals become the cast; their words, the lines", () => {
    const im = importScreenplay(SCREENPLAY);
    expect(im.characters).toEqual(["Mara", "Dev"]);
    expect(im.lines).toBe(4);
    const { script, errors } = parseScript(im.text);
    expect(errors).toEqual([]);
    expect(script.cast.map((c) => c.body)).toEqual(["female", "male"]);
    const said = script.steps.map((s) => (s.kind === "line" ? s.line.text : ""));
    expect(said).toEqual(["It's back. The same signal.", "Which one? We've had three this week.", "The one that hums.", "Then let's answer it."]);
  });

  test("NAME: words text imports too", () => {
    const im = importScreenplay("ANA: Hi.\nBEN: Hello, Ana.\nANA: Bye.");
    expect(im.characters).toEqual(["Ana", "Ben"]);
    expect(im.lines).toBe(3);
  });

  test("the cast and the words are capped", () => {
    const many = Array.from({ length: MAX_CAST + 3 }, (_, i) => `P${String.fromCharCode(65 + i)}: one two three`).join("\n");
    const a = importScreenplay(many);
    expect(a.characters.length).toBe(MAX_CAST);
    expect(a.dropped.characters.length).toBe(3);
    const long = Array.from({ length: 100 }, (_, i) => `${i % 2 ? "ANA" : "BEN"}: ${"word ".repeat(20)}`).join("\n");
    const b = importScreenplay(long);
    expect(b.words).toBeLessThanOrEqual(MAX_WORDS);
    expect(b.dropped.lines).toBeGreaterThan(0);
    expect(parseScript(b.text).errors).toEqual([]);
  });

  test("choosing who reads a part rewrites its cast line", () => {
    const im = importScreenplay(SCREENPLAY);
    const dev = parseScript(im.text).script.cast[1];
    const t = setVoice(im.text, dev, "child");
    const { script } = parseScript(t);
    expect(script.cast[1].body).toBe("child");
    expect(script.cast[1].pitch).toBe(dev.pitch);
  });
});

describe("dialogue render", () => {
  test("each voice speaks its own lines, in turn, and the shared line together", async () => {
    const { script } = parseScript("Ava = female\nTheo = male\nAva: Hello there.\nTheo: We waited.\nAva + Theo: Yes.");
    const d = await renderDialogue(script);
    const [ava, theo] = d.voices;
    expect(ava.tele.frames).toBe(Math.round((d.duration * SAMPLE_RATE) / HOP));
    expect(theo.tele.frames).toBe(ava.tele.frames);
    // turns: Theo starts after Ava's first line ends; the last line starts together
    expect(theo.lines[0].t0).toBeGreaterThan(ava.lines[0].t1);
    expect(ava.lines[1].t0).toBe(theo.lines[1].t0);
    expect(ava.lines[0].words.map((w) => w.text.toLowerCase())).toEqual(["hello", "there"]);
    // each head moves only while its owner talks
    const mid = (l: { t0: number; t1: number }) => (l.t0 + l.t1) / 2;
    const voicedAt = (v: typeof ava, t: number) => v.tele.message(v.tele.indexAt(t)).t.outRms;
    expect(voicedAt(ava, mid(ava.lines[0]))).toBeGreaterThan(10 * voicedAt(ava, mid(theo.lines[0])) + 1e-9);
    expect(voicedAt(theo, mid(theo.lines[0]))).toBeGreaterThan(10 * voicedAt(theo, mid(ava.lines[0])) + 1e-9);
    expect(lineAt(ava, mid(ava.lines[0]))?.line).toBe(ava.lines[0]);
    // the mix: a WAV of the whole dialogue, sounding, under full scale
    const pcm = new Int16Array(d.wav, 44);
    expect(pcm.length).toBe(Math.round(d.duration * SAMPLE_RATE));
    let peak = 0;
    for (const s of pcm) peak = Math.max(peak, Math.abs(s));
    expect(peak).toBeGreaterThan(8000);
    expect(peak).toBeLessThanOrEqual(32767 * 0.96);
  }, 60_000);
});
