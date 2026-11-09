/**
 * The dialogue page (`?dialogue`): write a script for several voices, render it, and watch a head for every participant say their lines, with
 * the words lit as they are said. Record saves the whole stage with the sound as one video.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { Screen } from "../ui/components/Screen.tsx";
import { AnatomyView3D, type HeadFeed } from "../ui/components/AnatomyView3D.tsx";
import { Transport } from "../show/transport.ts";
import { getModel } from "../ui/useModel.ts";
import { VoiceModel } from "../ui/analysis.ts";
import type { ViewFlags } from "../ui/render/anatomy.ts";
import { download, type Recording } from "../ui/record.ts";
import { EXAMPLE_SCRIPT, HUMAN_BODIES, MAX_CAST, MAX_WORDS, importScreenplay, parseScript, scriptWords, setMember, setVoice, type CastMember, type HumanBody } from "./script.ts";
import { EMOTIONS } from "./emotion.ts";
import { HUMAN_KNOBS, HUMAN_PRESETS, NO_TUNE, knobValue, type Knob, type Tune } from "./tune.ts";
import { Chip } from "../ui/components/Screen.tsx";
import { Slider } from "../ui/components/Controls.tsx";
import { lineAt, renderDialogue, scriptLineAt, type RenderedDialogue, type RenderedVoice, type ScriptLine } from "./render.ts";
import { recordStage, type StagePanel } from "./stageRecorder.ts";

const STORE_KEY = "chimera.dialogue.script";
const VIEW: ViewFlags = { pressure: true, particles: true, grid: false, calipers: false };

const clock = (t: number) => `${Math.floor(t / 60)}:${String(Math.floor(t % 60)).padStart(2, "0")}`;

const BODY_LABEL: Record<HumanBody, string> = { male: "Adult male", female: "Adult female", child: "Child" };
const voiceLabel = (m: CastMember) => BODY_LABEL[m.body];

export function DialoguePage() {
  const [text, setText] = useState(() => localStorage.getItem(STORE_KEY) ?? EXAMPLE_SCRIPT);
  const parsed = useMemo(() => parseScript(text), [text]);
  const [transport, setTransport] = useState<Transport | null>(null);
  const [show, setShow] = useState<RenderedDialogue | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [, setTick] = useState(0);
  const panels = useRef<(HTMLElement | null)[]>([]);
  const rec = useRef<Recording | null>(null);
  const abort = useRef<AbortController | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  /** the Script screen: the text to edit, or the lines as they play */
  const [view, setView] = useState<"edit" | "lines">("edit");
  const [withScript, setWithScript] = useState(true);
  const fileInput = useRef<HTMLInputElement>(null);

  /** a screenplay in: the characters become the cast, their lines the dialogue */
  const importText = (raw: string, from: string) => {
    const im = importScreenplay(raw);
    if (!im.lines) {
      setNotice(`No dialogue found in ${from}: put each character's name in capitals on its own line above their words, or write "NAME: words".`);
      return;
    }
    setText(im.text);
    const cut = im.dropped.lines ? ` Left out ${im.dropped.lines} line${im.dropped.lines === 1 ? "" : "s"} to stay within ${MAX_CAST} characters and ${MAX_WORDS} words${im.dropped.characters.length ? ` (${im.dropped.characters.slice(0, 4).join(", ")}${im.dropped.characters.length > 4 ? "…" : ""})` : ""}.` : "";
    setNotice(`${from}: ${im.characters.length} character${im.characters.length === 1 ? "" : "s"}, ${im.lines} lines, ${im.words} words.${cut} Choose who reads each part below.`);
  };
  const openFile = async (f: File | undefined) => {
    if (!f) return;
    if (f.size > 2_000_000) return setNotice(`${f.name} is too big: a script up to 2 MB, as plain text (.txt or .fountain).`);
    importText(await f.text(), f.name);
  };

  useEffect(() => localStorage.setItem(STORE_KEY, text), [text]);
  useEffect(() => {
    const t = new Transport();
    setTransport(t);
    return () => t.dispose();
  }, []);
  useEffect(() => {
    if (!transport) return;
    const off = transport.subscribe(() => setTick((n) => n + 1));
    const id = setInterval(() => transport.isPlaying && setTick((n) => n + 1), 250);
    return () => {
      off();
      clearInterval(id);
    };
  }, [transport]);

  const render = async () => {
    if (!transport || parsed.errors.length) return;
    abort.current?.abort();
    const ac = new AbortController();
    abort.current = ac;
    transport.pause();
    setError(null);
    setProgress(0);
    try {
      const d = await renderDialogue(parsed.script, setProgress, ac.signal);
      await transport.load(d.wav.slice(0));
      transport.seek(0);
      panels.current = [];
      setShow(d);
    } catch (e) {
      if (!(e instanceof DOMException && e.name === "AbortError")) setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (abort.current === ac) setProgress(null);
    }
  };

  const stopRecording = () => {
    rec.current?.stop();
    rec.current = null;
    setRecording(false);
  };

  const record = () => {
    if (!transport || !show) return;
    if (rec.current) return stopRecording();
    const stage: StagePanel[] = show.voices.map((voice, i) => ({ voice, el: panels.current[i]!, sub: voiceLabel(voice.member) })).filter((p) => p.el);
    transport.pause();
    transport.seek(0);
    const stamp = new Date().toISOString().slice(0, 19).replace(/[T:]/g, "-");
    const r = recordStage(stage, transport, (file, ext) => download(file, `chimera-dialogue-${stamp}.${ext}`), withScript ? show.script : undefined);
    setView("lines");
    if (!r) return setError("This browser can't record video");
    rec.current = r;
    setRecording(true);
    transport.play();
    // a hidden tab stops drawing (the browser pauses animation), so the video would freeze
    const onHidden = () => document.hidden && rec.current === r && setNotice("The video stops moving while this tab is hidden: keep it in front until the recording finishes, or record again.");
    document.addEventListener("visibilitychange", onHidden);
    // stop with the dialogue
    const off = transport.subscribe(() => {
      if (!transport.isPlaying && rec.current === r) {
        off();
        document.removeEventListener("visibilitychange", onHidden);
        stopRecording();
      }
    });
  };

  const st = transport?.state ?? { playing: false, time: 0, duration: 0, epoch: 0 };
  const castNow = show?.voices.length ?? parsed.script.cast.length;
  return (
    <div className="dialogue">
      <header className="duet-top">
        <a className="wordmark" href="./">
          Chimera
        </a>
        <span className="duet-title">Dialogue</span>
      </header>
      <div className="dialogue-main">
        <Screen
          className="dialogue-script"
          title="Script"
          sub={view === "edit" ? "Cast, then lines" : "As it plays"}
          tools={
            <span className="keys-mode" role="radiogroup" aria-label="Script view">
              <Chip toggle={false} role="radio" aria-checked={view === "edit"} on={view === "edit"} onClick={() => setView("edit")}>
                Edit
              </Chip>
              <Chip toggle={false} role="radio" aria-checked={view === "lines"} on={view === "lines"} disabled={!show} onClick={() => setView("lines")} title={show ? "The lines as they play: click one to go there" : "Render first"}>
                Lines
              </Chip>
            </span>
          }
        >
          {view === "lines" && show && transport ? (
            <ScriptLines lines={show.script} transport={transport} />
          ) : (
          <div className="dialogue-editor">
            <textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              spellCheck={false}
              aria-label="Dialogue script"
              placeholder="Write a script, or drop a screenplay file here"
              onKeyDown={(e) => e.key === "Enter" && (e.metaKey || e.ctrlKey) && void render()}
              onDragOver={(e) => e.dataTransfer.types.includes("Files") && e.preventDefault()}
              onDrop={(e) => {
                if (!e.dataTransfer.files.length) return;
                e.preventDefault();
                void openFile(e.dataTransfer.files[0]);
              }}
            />
            <div className="dialogue-actions">
              <button className="ghost" onClick={() => fileInput.current?.click()} title="A screenplay as plain text (.txt, .fountain)">
                Open screenplay…
              </button>
              <button className="ghost" onClick={() => importText(text, "The pasted text")} title="Turn a screenplay pasted above into a cast and lines">
                Convert pasted screenplay
              </button>
              <input ref={fileInput} type="file" accept=".txt,.fountain,.md,.text,text/plain" hidden onChange={(e) => void openFile(e.target.files?.[0]).finally(() => (e.target.value = ""))} />
            </div>
            {notice && <p className="dim dialogue-notice">{notice}</p>}
            <Cast cast={parsed.script.cast} onVoice={(m, v) => setText((t) => setVoice(t, m, v))} onTune={(m, tune) => setText((t) => setMember(t, m.name, m.body, tune))} words={scriptWords(parsed.script)} />
            {parsed.errors.length > 0 && (
              <ul className="dialogue-errors">
                {parsed.errors.map((er, i) => (
                  <li key={i}>{er.line ? `Line ${er.line}: ${er.message}` : er.message}</li>
                ))}
              </ul>
            )}
            {error && <p className="dialogue-errors">{error}</p>}
            <div className="dialogue-actions">
              <button className="primary" onClick={() => void render()} disabled={!!parsed.errors.length || progress !== null}>
                {progress !== null ? `Rendering ${Math.round(progress * 100)}%` : show ? "Render again" : "Render"}
              </button>
              {progress !== null && (
                <button className="ghost" onClick={() => abort.current?.abort()}>
                  Cancel
                </button>
              )}
            </div>
            <p className="dim dialogue-help">
              <b>Name = voice</b> adds someone: male, female or child; then presets and settings to shape it, like <b>gravelly slow pitch 95</b> (Tune shows them all, with sliders). <b>Name: words</b> is a line; <b>Ava + Theo:</b> say it together; <b>Ava (very sad):</b> how it's said ({Object.keys(EMOTIONS).join(", ")}, or words like upset or sobbing; <i>a little</i> or <i>very</i> for how much);{" "}
              <b>Name sings:</b> holds one pitch; <b>(pause 1)</b> waits.
            </p>
          </div>
          )}
        </Screen>
        <main className="dialogue-stage" style={{ "--voices": Math.min(4, Math.max(1, castNow)), "--rows": castNow > 4 ? 2 : 1 } as React.CSSProperties}>
          {show && transport ? (
            show.voices.map((v, i) => <Participant key={`${show.duration}-${i}`} voice={v} index={i} count={show.voices.length} show={show} transport={transport} panelRef={(el) => (panels.current[i] = el)} />)
          ) : (
            <p className="duet-msg">{progress !== null ? "Rendering the voices…" : `Render the script to see ${parsed.script.cast.length || "the"} participant${parsed.script.cast.length === 1 ? "" : "s"} talk.`}</p>
          )}
        </main>
      </div>
      <footer className="duet-bar">
        <button className="primary large" disabled={!show || recording} onClick={() => (st.playing ? transport?.pause() : (setView("lines"), transport?.play()))}>
          {st.playing ? "Pause" : st.duration > 0 && st.time >= st.duration - 0.05 ? "Again" : "Play"}
        </button>
        <span className="duet-time">
          {clock(st.time)} / {clock(st.duration)}
        </span>
        <input
          type="range"
          aria-label="Position in the dialogue"
          min={0}
          max={st.duration || 1}
          step={0.05}
          value={st.time}
          disabled={!show || recording}
          onChange={(e) => transport?.seek(Number(e.target.value))}
          style={{ "--pos": `${st.duration ? (100 * st.time) / st.duration : 0}%` } as React.CSSProperties}
        />
        <label className="dialogue-with-script" title="Put the script beside the heads in the video, the line being said lit">
          <input type="checkbox" checked={withScript} onChange={(e) => setWithScript(e.target.checked)} disabled={recording} /> Script in video
        </label>
        <button className={recording ? "primary large" : "ghost"} disabled={!show} onClick={record} title="Play the dialogue from the start and save it, every head and the sound, as a video (keep this tab in front while it records)">
          {recording ? "● Stop recording" : "● Record video"}
        </button>
      </footer>
    </div>
  );
}

/** Who reads each part: a voice for every character, and its tuning. */
function Cast(props: { cast: CastMember[]; onVoice: (m: CastMember, body: HumanBody) => void; onTune: (m: CastMember, t: Tune) => void; words: number }) {
  const [open, setOpen] = useState<string | null>(null);
  if (!props.cast.length) return null;
  return (
    <div className="dialogue-cast">
      {props.cast.map((m) => (
        <div key={m.name} className="dialogue-cast-member">
          <div className="dialogue-cast-row">
            <span className="dialogue-cast-name">{m.name}</span>
            <select value={m.body} onChange={(e) => props.onVoice(m, e.target.value as HumanBody)} aria-label={`Who reads ${m.name}`}>
              {HUMAN_BODIES.map((b) => (
                <option key={b} value={b}>
                  {BODY_LABEL[b]}
                </option>
              ))}
            </select>
            <button className={`ghost dialogue-tune-toggle${open === m.name ? " on" : ""}`} aria-expanded={open === m.name} onClick={() => setOpen(open === m.name ? null : m.name)} title={`Tune how ${m.name} sounds`}>
              Tune{m.tune.presets.length + Object.keys(m.tune.set).length ? " •" : ""}
            </button>
            <Hear member={m} />
          </div>
          {open === m.name && <TunePanel member={m} onTune={(t) => props.onTune(m, t)} />}
        </div>
      ))}
      <span className="dim dialogue-cast-count">
        {props.cast.length}/{MAX_CAST} cast · {props.words}/{MAX_WORDS} words
      </span>
    </div>
  );
}

const fmtKnob = (k: Knob) => (v: number) => (k.unit === "Hz" ? `${v.toFixed(0)} Hz` : k.unit === "×" ? `${v.toFixed(2)}×` : k.unit ? `${v.toFixed(1)} ${k.unit}` : `${Math.round(v * 100)}%`);

/** Presets to toggle, and a slider for every setting (a slider moved writes the setting into the cast line). */
function TunePanel({ member, onTune }: { member: CastMember; onTune: (t: Tune) => void }) {
  const t = member.tune;
  const togglePreset = (p: string) => onTune({ ...t, presets: t.presets.includes(p) ? t.presets.filter((x) => x !== p) : [...t.presets, p] });
  return (
    <div className="dialogue-tune">
      <div className="dialogue-presets" role="group" aria-label={`Presets for ${member.name}`}>
        {Object.entries(HUMAN_PRESETS).map(([p, d]) => (
          <Chip key={p} on={t.presets.includes(p)} onClick={() => togglePreset(p)} title={d.about}>
            {p}
          </Chip>
        ))}
      </div>
      <div className="dialogue-knobs">
        {HUMAN_KNOBS.map((k) => (
          <div key={k.key} title={k.hint} className={t.set[k.key] !== undefined ? "set" : undefined}>
            <Slider label={k.label} value={knobValue(k, t, member.body)} min={k.min} max={k.max} log={k.log} onChange={(v) => onTune({ ...t, set: { ...t.set, [k.key]: Number(v.toFixed(k.unit === "Hz" ? 0 : 2)) } })} format={fmtKnob(k)} />
          </div>
        ))}
      </div>
      <button className="ghost" onClick={() => onTune(NO_TUNE)} disabled={!t.presets.length && !Object.keys(t.set).length}>
        Reset {member.name}'s voice
      </button>
    </div>
  );
}

/** Hear a character say a sample line, as tuned. */
function Hear({ member }: { member: CastMember }) {
  const [busy, setBusy] = useState(false);
  const audio = useRef<HTMLAudioElement | null>(null);
  const hear = async () => {
    setBusy(true);
    try {
      const d = await renderDialogue({ cast: [member], steps: [{ kind: "line", line: { speakers: [0], text: `Hello. My name is ${member.name}, and this is how I sound.`, sung: false } }] });
      audio.current?.pause();
      const url = URL.createObjectURL(new Blob([d.wav], { type: "audio/wav" }));
      const a = new Audio(url);
      a.onended = () => URL.revokeObjectURL(url);
      audio.current = a;
      await a.play();
    } finally {
      setBusy(false);
    }
  };
  return (
    <button className="ghost" onClick={() => void hear()} disabled={busy} title={`Hear ${member.name} say a sample line`} aria-label={`Hear ${member.name}`}>
      {busy ? "…" : "▶"}
    </button>
  );
}

/** The script as it plays: every line, the one being said lit and kept in view; click one to go there. */
function ScriptLines({ lines, transport }: { lines: readonly ScriptLine[]; transport: Transport }) {
  const [now, setNow] = useState(-1);
  const box = useRef<HTMLOListElement>(null);
  useEffect(() => {
    let raf = 0;
    const loop = () => {
      const i = scriptLineAt(lines, transport.time());
      setNow((p) => (p === i ? p : i));
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [lines, transport]);
  useEffect(() => {
    const el = box.current, item = el?.children[now] as HTMLElement | undefined;
    if (!el || !item) return;
    // scroll the list only (not the page): the line a third of the way down
    el.scrollTo({ top: Math.max(0, item.offsetTop - el.offsetTop - el.clientHeight / 3), behavior: "smooth" });
  }, [now]);
  return (
    <ol className="dialogue-lines" ref={box}>
      {lines.map((l, i) => (
        <li key={i} className={i === now ? "now" : now >= 0 && i < now ? "said" : undefined} aria-current={i === now ? "true" : undefined}>
          <button onClick={() => transport.seek(l.t0)} title="Go to this line">
            <span className="dialogue-line-who">
              {l.speakers.join(" + ")}
              {l.mood && <span className="dialogue-mood"> ({l.mood})</span>}
            </span>
            <span className="dialogue-line-text">{l.text}</span>
          </button>
        </li>
      ))}
    </ol>
  );
}

/** One participant: a 3D head, and what they're saying. */
function Participant(props: { voice: RenderedVoice; index: number; count: number; show: RenderedDialogue; transport: Transport; panelRef: (el: HTMLElement | null) => void }) {
  const { voice, show, transport } = props;
  const m = voice.member;
  // the left half look right, the right half look left: toward each other
  const facing: 1 | -1 = props.count > 1 && props.index >= props.count / 2 ? -1 : 1;
  const humanFeed = useRef<HeadFeed | null>(null);
  if (!humanFeed.current) {
    const track = voice.tele;
    humanFeed.current = {
      model: getModel(track.body, track.sampleRate),
      view: VIEW,
      maxDpr: 1.5,
      facing,
      frame: () => {
        const msg = track.message(track.indexAt(transport.time()));
        return { tele: transport.isPlaying ? msg : null, artic: VoiceModel.articFromArray(msg.t.artic), epoch: transport.epoch };
      },
    };
  }
  return (
    <div className="dialogue-participant" ref={props.panelRef}>
      <Screen className="duet-head" focus title={m.name} sub={voiceLabel(m)}>
        <AnatomyView3D feed={humanFeed.current} readouts={false} />
      </Screen>
      <Caption voice={voice} transport={transport} />
    </div>
  );
}

/** The line being said, its word lit. */
function Caption({ voice, transport }: { voice: RenderedVoice; transport: Transport }) {
  const [now, setNow] = useState<ReturnType<typeof lineAt>>(null);
  useEffect(() => {
    let raf = 0;
    const loop = () => {
      const n = lineAt(voice, transport.time());
      setNow((prev) => (prev?.line === n?.line && prev?.word === n?.word ? prev : n));
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [voice, transport]);
  return (
    <div className="dialogue-caption" aria-live="polite">
      {now ? (
        <>
          <p>
            {now.line.mood && <span className="dim dialogue-mood">({now.line.mood}) </span>}
            {now.line.words.map((w, i) => (
              <span key={i} className={`dialogue-word${i === now.word ? " now" : ""}`}>
                {w.text}{" "}
              </span>
            ))}
          </p>
        </>
      ) : (
        <p className="dim">&nbsp;</p>
      )}
    </div>
  );
}
