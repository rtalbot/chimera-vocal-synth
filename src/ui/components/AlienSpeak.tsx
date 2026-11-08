/**
 * An invented creature speaks: English text in, an utterance in the alien's own language out
 * (crates/chimera/src/creatures/language.rs). The same word always becomes the same alien word.
 * While it plays, the word being said is lit in both the English and the alien.
 */
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useStore } from "../store.ts";
import { alienLanguage, alienSpeak } from "../../wasm/engine.ts";
import type { AlienUtterance, CreatureDef } from "../../wasm/types.ts";
import { engine } from "../../audio/AudioEngine.ts";
import { useLiveCreature } from "../creatureLive.ts";
import { Slider } from "./Controls.tsx";
import { onSungWord, setSungText } from "../alienSing.ts";

/** The engine marks a syllable's tone with a combining accent on its vowel (acute high, grave
 *  low). The stroke font has no accents, so a toned vowel is drawn raised or lowered instead. */
function toned(alien: string): ReactNode[] {
  const out: ReactNode[] = [];
  let plain = "";
  const chars = [...alien.normalize("NFD")];
  for (let i = 0; i < chars.length; i++) {
    const tone = chars[i + 1] === "\u0301" ? "hi" : chars[i + 1] === "\u0300" ? "lo" : null;
    if (!tone) {
      if (chars[i] !== "\u0301" && chars[i] !== "\u0300") plain += chars[i];
      continue;
    }
    if (plain) out.push(plain), (plain = "");
    out.push(<span key={i} className={`tone-${tone}`}>{chars[i]}</span>);
  }
  if (plain) out.push(plain);
  return out;
}

const FIRST_WORDS = "Hello, traveller. Where are you going? We have waited for you!";

export function AlienSpeak({ def }: { def: CreatureDef }) {
  const lang = useMemo(() => alienLanguage(def.id), [def.id]);
  const start = useStore((s) => s.start);
  const [text, setText] = useState(FIRST_WORDS);
  const [rate, setRate] = useState(1);
  const [said, setSaid] = useState<{ u: AlienUtterance; at: number } | null>(null);
  const [now, setNow] = useState(0);
  const live = useLiveCreature(def.id);
  const speaking = live?.call === "speak";

  // a new creature: forget what the last one said
  useEffect(() => setSaid(null), [def.id]);
  // the keys sing from this text ("Keys play: Words"); show which word they sang last
  const [sung, setSung] = useState(-1);
  useEffect(() => setSungText(text), [text]);
  useEffect(() => onSungWord(setSung), []);
  const sungWord = sung >= 0 ? text.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w))[sung] : undefined;

  // follow the words while it speaks
  useEffect(() => {
    if (!said) return;
    let raf = 0;
    const tick = () => {
      const t = (performance.now() - said.at) / 1000;
      setNow(t);
      if (t < said.u.duration + 0.2) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [said]);

  if (!lang) return null;

  const speak = async () => {
    const u = alienSpeak(def.id, text, { rate });
    if (!u) return;
    await start();
    if (!useStore.getState().started) return;
    engine.speakAlien(u.flat);
    setSaid({ u, at: performance.now() });
  };

  const current = said && speaking ? said.u.words.findIndex((w) => now >= w.t0 && now < w.t1 + 0.05) : -1;

  return (
    <div className="say alien-say">
      <p className="dim alien-lang">
        <span className="alien-lang-name">{lang.name}</span> {lang.about}
      </p>
      <div className="say-row">
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void speak();
            e.stopPropagation();
          }}
          rows={2}
          aria-label={`Text for the ${def.name} to say in ${lang.name}`}
          placeholder={`Type something for the ${def.name} to say`}
        />
        <div className="say-actions">
          <button className="primary" onClick={() => void speak()}>
            Speak
          </button>
          <button className="ghost" onClick={() => engine.stopCall()}>
            Stop
          </button>
        </div>
      </div>
      <div className="say-row">
        <p className="transcription alien-words" aria-live="polite">
          {said
            ? said.u.words.map((w, i) => (
                <span key={i} className={`alien-word${i === current ? " now" : ""}`} title={w.text}>
                  {toned(w.alien)}{" "}
                </span>
              ))
            : `Say it in ${lang.name}: the translation appears here.`}
        </p>
        <div className="say-rate">
          <Slider label="Rate" value={rate} min={0.5} max={2} onChange={setRate} format={(v) => `${v.toFixed(2)}×`} />
        </div>
      </div>
      {sungWord && (
        <p className="dim alien-english">
          Keys sang <span className="alien-word now">{sungWord}</span>
        </p>
      )}
      {said && (
        <p className="dim alien-english">
          {said.u.words.map((w, i) => (
            <span key={i} className={`alien-word${i === current ? " now" : ""}`}>
              {w.text}{" "}
            </span>
          ))}
        </p>
      )}
    </div>
  );
}
