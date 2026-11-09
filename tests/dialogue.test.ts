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
const { renderDialogue, lineAt, scriptLineAt, HOP, SAMPLE_RATE } = await import("../src/dialogue/render.ts");
const { g2p } = await import("../src/ui/g2p.ts");
const await_tune = await import("../src/dialogue/tune.ts");
const await_emotion = await import("../src/dialogue/emotion.ts");
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
    expect(script.cast[1].tune.set.pitch).toBe(100);
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
    expect(script.cast[1].tune).toEqual(dev.tune);
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
    // the script as it plays: every line in order, who says it, and when
    expect(d.script.map((l) => [l.speakers.join("+"), l.text])).toEqual([["Ava", "Hello there."], ["Theo", "We waited."], ["Ava+Theo", "Yes."]]);
    expect(d.script.map((_, i) => scriptLineAt(d.script, mid(d.script[i])))).toEqual([0, 1, 2]);
    expect(scriptLineAt(d.script, 0.1)).toBe(-1);
    // the mix: a WAV of the whole dialogue, sounding, under full scale
    const pcm = new Int16Array(d.wav, 44);
    expect(pcm.length).toBe(Math.round(d.duration * SAMPLE_RATE));
    let peak = 0;
    for (const s of pcm) peak = Math.max(peak, Math.abs(s));
    expect(peak).toBeGreaterThan(8000);
    expect(peak).toBeLessThanOrEqual(32767 * 0.96);
  }, 60_000);
});

describe("voice tuning", () => {
  const { parseTune, formatTune, humanSettings } = await_tune;

  test("presets and settings on a cast line; a word that is both is a setting when a number follows", () => {
    const { tune, errors } = parseTune(["gravelly", "slow", "pitch", "95", "breathy", "0.6", "bright"]);
    expect(errors).toEqual([]);
    expect(tune.presets).toEqual(["gravelly", "slow", "bright"]);
    expect(tune.set).toEqual({ pitch: 95, breathy: 0.6 });
    expect(parseTune(formatTune(tune).split(" ")).tune).toEqual(tune);
    // explicit settings win over presets; values are kept in range
    const s = humanSettings("male", parseTune(["gravelly", "rough", "0.1", "rate", "9"]).tune);
    expect(s.source.roughness).toBe(0.1);
    expect(s.rate).toBe(2);
    expect(parseTune(["wobbly"]).errors.length).toBe(1);
    expect(parseTune(["pitch"]).errors.length).toBe(1);
  });

  test("an imported screenplay tells apart characters given the same voice", () => {
    const im = importScreenplay("ANA: One.\nBEA: Two.\nCAL: Three.\nDEE: Four.");
    const cast = parseScript(im.text).script.cast;
    expect(cast.map((c) => c.body)).toEqual(["female", "male", "female", "male"]);
    expect(cast[0].tune.set).toEqual({});
    expect(cast[2].tune.set.pitch).toBeDefined();
    expect(cast[2].tune.set.pitch).not.toBe(humanSettings("female", cast[0].tune).f0);
  });

  // each setting reaches the sound: one short line, measured against the untuned voice
  const LINE = "Where were you yesterday evening?";
  async function said(cast: string) {
    const { script, errors } = parseScript(`A = ${cast}\nA: ${LINE}`);
    expect(errors).toEqual([]);
    const d = await renderDialogue(script);
    const v = d.voices[0], l = v.lines[0];
    const pcm = new Int16Array(d.wav, 44);
    let e = 0, de = 0;
    for (let i = Math.floor(l.t0 * SAMPLE_RATE) + 1; i < l.t1 * SAMPLE_RATE; i++) {
      e += pcm[i] ** 2;
      de += (pcm[i] - pcm[i - 1]) ** 2;
    }
    const frames = Array.from({ length: v.tele.indexAt(l.t1) - v.tele.indexAt(l.t0) }, (_, k) => v.tele.message(v.tele.indexAt(l.t0) + k).t);
    const voiced = frames.filter((f) => f.voicing > 0.5);
    const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / Math.max(1, xs.length);
    return { dur: l.t1 - l.t0, f0: mean(voiced.map((f) => f.f0)), phLen: mean(frames.map((f) => f.phLen)), hiss: de / e };
  }

  test("speed, pitch and larynx", async () => {
    const plain = await said("male");
    expect((await said("male rate 1.5")).dur).toBeLessThan(plain.dur * 0.8);
    expect((await said("male pitch 80")).f0).toBeLessThan(plain.f0 * 0.8);
    expect((await said("male larynx -1")).phLen).toBeGreaterThan(plain.phLen + 0.5);
    expect((await said("male larynx 1")).phLen).toBeLessThan(plain.phLen - 0.5);
  }, 60_000);

  test("breathy and pressed voices are brighter in the top of the spectrum", async () => {
    const plain = await said("male");
    expect((await said("male breathy 0.8")).hiss).toBeGreaterThan(plain.hiss * 1.1);
    expect((await said("male tense 0.9")).hiss).toBeGreaterThan(plain.hiss * 1.1);
  }, 60_000);
});

describe("emotion", () => {
  const { parseMood, moodShift, emotionOf, formatMood } = await_emotion;

  test("emotions, everyday words and adverbs, strengths, and mixtures", () => {
    expect(emotionOf("sobbing")).toBe("crying");
    expect(emotionOf("angrily")).toBe("angry");
    expect(emotionOf("Excitedly")).toBe("excited");
    expect(emotionOf("to Dev")).toBeNull();
    const { mood, unknown } = parseMood("a little sad, very angry, whispering 0.3, to Dev");
    expect(mood!.parts).toEqual([
      { emotion: "sad", strength: 0.4 },
      { emotion: "angry", strength: 1 },
      { emotion: "whispering", strength: 0.3 },
    ]);
    expect(unknown).toEqual(["to Dev"]);
    expect(parseMood(formatMood(mood!)).mood).toEqual(mood);
    // stronger is further: pitch up more, faster
    const some = moodShift(parseMood("a little excited").mood), lots = moodShift(parseMood("very excited").mood);
    expect(lots.pitchBy).toBeGreaterThan(some.pitchBy);
    expect(some.pitchBy).toBeGreaterThan(1);
  });

  test("a line's mood: after the speaker, at the start of the words, or not an emotion", () => {
    const { script, errors } = parseScript("Ava = female\nAva (very sad): I know.\nAva: (sobbing) I know.\nAva (O.S.): Hm.");
    expect(script.steps.slice(0, 2).map((s) => s.kind === "line" && s.line.mood?.parts[0])).toEqual([
      { emotion: "sad", strength: 1 },
      { emotion: "crying", strength: 0.75 },
    ]);
    expect(script.steps[1].kind === "line" && script.steps[1].line.text).toBe("I know.");
    expect(errors.map((e) => e.line)).toEqual([4]);
  });

  test("a screenplay's directions become moods, and a new one splits the speech", () => {
    const im = importScreenplay(`                    MARA
          (quietly)
          It's back.
          (beat)
          The same signal.
          (suddenly furious)
          Why didn't you tell me?

                    DEV
          (to Mara)
          I didn't know.`);
    const lines = parseScript(im.text).script.steps.map((s) => (s.kind === "line" ? [s.line.text, s.line.mood?.parts.map((p) => p.emotion).join("+") ?? ""] : null));
    expect(lines).toEqual([
      ["It's back. The same signal.", "calm"],
      ["Why didn't you tell me?", "furious"],
      ["I didn't know.", ""],
    ]);
  });

  test("a mood changes how the line sounds, for that line only", async () => {
    const { script } = parseScript("A = male\nA: Where were you yesterday?\nA (furious): Where were you yesterday?\nA (very sad): Where were you yesterday?\nA: Where were you yesterday?\nA (whispering): Where were you yesterday?");
    const d = await renderDialogue(script);
    const v = d.voices[0], tr = v.tele;
    const pcm = new Int16Array(d.wav, 44);
    const at = (k: number) => {
      const l = v.lines[k];
      let e = 0, de = 0;
      for (let i = Math.floor(l.t0 * SAMPLE_RATE) + 1; i < l.t1 * SAMPLE_RATE; i++) {
        e += pcm[i] ** 2;
        de += (pcm[i] - pcm[i - 1]) ** 2;
      }
      const fr: number[] = [];
      for (let i = tr.indexAt(l.t0); i < tr.indexAt(l.t1); i++) {
        const t = tr.message(i).t;
        if (t.voicing > 0.5) fr.push(t.f0);
      }
      return { dur: l.t1 - l.t0, rms: Math.sqrt(e / ((l.t1 - l.t0) * SAMPLE_RATE)), bright: de / e, f0: fr.reduce((a, b) => a + b, 0) / Math.max(1, fr.length), voiced: fr.length };
    };
    const [plain, furious, sad, again, whisper] = [0, 1, 2, 3, 4].map(at);
    expect(furious.rms).toBeGreaterThan(plain.rms * 1.5);
    expect(furious.bright).toBeGreaterThan(plain.bright * 1.5);
    expect(sad.f0).toBeLessThan(plain.f0 * 0.95);
    expect(sad.dur).toBeGreaterThan(plain.dur * 1.1);
    expect(sad.rms).toBeLessThan(plain.rms);
    // the mood lasts its line: said plainly again, it's as before
    expect(Math.abs(again.f0 / plain.f0 - 1)).toBeLessThan(0.02);
    expect(again.rms / plain.rms).toBeGreaterThan(0.9);
    expect(again.rms / plain.rms).toBeLessThan(1.1);
    // a whisper has no voice in it
    expect(whisper.voiced).toBeLessThan(plain.voiced * 0.2);
    expect(v.lines[1].mood).toBe("furious");
  }, 60_000);
});
