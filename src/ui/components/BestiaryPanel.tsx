import { memo, useState } from "react";
import { useStore } from "../store.ts";
import { CREATURES, creatureById } from "../../wasm/engine.ts";
import type { CreatureDef } from "../../wasm/types.ts";
import { useLiveCreature } from "../creatureLive.ts";
import { Keys } from "./Keys.tsx";
import { Chip, Screen } from "./Screen.tsx";
import { EncoderPages, type EncoderPage } from "./Encoder.tsx";
import { Spectrogram } from "./Spectrogram.tsx";
import { CreatureTubes } from "./CreatureTubes.tsx";
import { AlienSpeak } from "./AlienSpeak.tsx";
import { SING_WORDS } from "../alienSing.ts";

/** Thumbnail: the creature's outline in blue with its airway traced in amber. */
const Thumb = memo(function Thumb({ def }: { def: CreatureDef }) {
  const v = def.visual;
  const vw = v.view.x1 - v.view.x0, vh = v.view.y1 - v.view.y0;
  const pad = Math.max(vw, vh) * 0.06;
  const stroke = Math.max(vw, vh) * 0.012;
  const pts = (p: [number, number][]) => p.map(([x, y]) => `${x},${-y}`).join(" ");
  return (
    <svg className="thumb" viewBox={`${v.view.x0 - pad} ${-v.view.y1 - pad} ${vw + 2 * pad} ${vh + 2 * pad}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <polygon points={pts(v.silhouette)} fill="none" stroke="var(--blue)" strokeWidth={stroke} strokeLinejoin="round" />
      {(v.parts ?? []).map((p, i) => (
        <polygon key={i} points={pts(p.pts)} fill="none" stroke="var(--blue)" strokeWidth={stroke * 0.7} strokeLinejoin="round" />
      ))}
      {v.tubes.map((t) => (
        <polyline key={t.tube} points={pts(t.path)} fill="none" stroke="var(--amber)" strokeWidth={stroke * 0.9} strokeLinecap="round" strokeLinejoin="round" />
      ))}
    </svg>
  );
});

/** The Bestiary's two sections: real animals, and the invented aliens (engine group "fantasy"). */
type Section = "creatures" | "aliens";
const sectionOf = (c: CreatureDef | undefined): Section => (c?.group === "fantasy" ? "aliens" : "creatures");
const SECTIONS: { id: Section; label: string; list: CreatureDef[] }[] = [
  { id: "creatures", label: "Creatures", list: CREATURES.filter((c) => sectionOf(c) === "creatures") },
  { id: "aliens", label: "Aliens", list: CREATURES.filter((c) => sectionOf(c) === "aliens") },
];

function SectionTabs({ section, onSection }: { section: Section; onSection: (s: Section) => void }) {
  return (
    <span className="keys-mode" role="radiogroup" aria-label="Which specimens to show">
      {SECTIONS.map((s) => (
        <Chip key={s.id} toggle={false} role="radio" aria-checked={section === s.id} on={section === s.id} onClick={() => onSection(s.id)}>
          {`${s.label} ${s.list.length}`}
        </Chip>
      ))}
    </span>
  );
}

function Gallery({ section }: { section: Section }) {
  const creature = useStore((s) => s.creature);
  const setCreature = useStore((s) => s.setCreature);
  const list = SECTIONS.find((s) => s.id === section)!.list;
  // one row up to five; more wrap into two rows of drawings
  const cols = Math.max(4, list.length <= 5 ? list.length : Math.ceil(list.length / 2));
  return (
    <div className="specimens" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, gridAutoRows: "minmax(0, 1fr)" }}>
      {list.map((c) => (
        <button key={c.id} className={`specimen${c.id === creature ? " on" : ""}`} onClick={() => setCreature(c.id)} aria-pressed={c.id === creature}>
          <Thumb def={c} />
          <span className="specimen-name">{c.name}</span>
          <span className="specimen-latin">{c.latin}</span>
        </button>
      ))}
    </div>
  );
}

function Calls({ def }: { def: CreatureDef }) {
  const playCall = useStore((s) => s.playCall);
  const hold = useStore((s) => s.hold);
  const setHold = useStore((s) => s.setHold);
  const keyCall = useStore((s) => s.keyCall);
  const setKeyCall = useStore((s) => s.setKeyCall);
  const live = useLiveCreature(def.id);
  return (
    <div className="calls-body">
      <div className="calls">
        {def.calls.map((c) => (
          <button key={c.id} className={`call${live?.call === c.id ? " playing" : ""}`} onClick={() => playCall(c.id)} title={c.hint}>
            <span className="call-name">{c.name}</span>
          </button>
        ))}
        <button className={`call sustain${hold ? " on" : ""}`} onClick={() => setHold(!hold)} aria-pressed={hold} title="Keep the breath on and shape the sound with the controls">
          <span className="call-name">{hold ? "Stop sustaining" : "Sustain"}</span>
        </button>
      </div>
      <div className="keys-mode" role="radiogroup" aria-label="What the keys play">
        <span className="dim">Keys play</span>
        <Chip toggle={false} role="radio" aria-checked={keyCall === null} on={keyCall === null} onClick={() => setKeyCall(null)}>
          Held voice
        </Chip>
        {def.calls.map((c) => (
          <Chip key={c.id} toggle={false} role="radio" aria-checked={keyCall === c.id} on={keyCall === c.id} onClick={() => setKeyCall(c.id)}>
            {c.name}
          </Chip>
        ))}
        {def.group === "fantasy" && (
          <Chip toggle={false} role="radio" aria-checked={keyCall === SING_WORDS} on={keyCall === SING_WORDS} onClick={() => setKeyCall(SING_WORDS)} title="Each key sings the next word of the Speak text, on its pitch">
            Words
          </Chip>
        )}
      </div>
      <p className="creature-blurb">{def.blurb}</p>
    </div>
  );
}

function fmt(v: number, p: CreatureDef["params"][number]): { value: string; unit: string } {
  const digits = p.digits ?? (p.max - p.min <= 2 ? 2 : v < 100 && p.log ? 1 : 0);
  return { value: v.toFixed(digits), unit: p.unit ?? "" };
}

function Params({ def }: { def: CreatureDef }) {
  const cparams = useStore((s) => s.cparams[def.id]);
  const setCParam = useStore((s) => s.setCParam);
  const live = useLiveCreature(def.id);
  // while a call plays, the encoders follow its automation; afterwards they return to yours
  const calling = !!live?.call;
  const pages: EncoderPage[] = [];
  for (let i = 0; i < def.params.length; i += 4) {
    pages.push({
      name: String(pages.length + 1),
      encoders: def.params.slice(i, i + 4).map((p) => {
        const mine = cparams?.[p.key] ?? p.default;
        return { label: p.label, value: calling ? (live!.params[p.key] ?? mine) : mine, min: p.min, max: p.max, log: p.log, onChange: (v: number) => setCParam(p.key, v), format: (v: number) => fmt(v, p) };
      }),
    });
  }
  return (
    <>
      {/* a sentence, too long for the header: it wraps here instead of being cut off there */}
      {def.role && <p className="cparams-role">{def.role}</p>}
      <EncoderPages key={def.id} label={`${def.name} controls`} pages={pages} />
    </>
  );
}

/** Bestiary's right column: the creatures, the waterfall, the calls, the controls, the airway, the keys. */
export function BestiaryRight() {
  const creature = useStore((s) => s.creature);
  const def = creatureById(creature);
  const [section, setSection] = useState<Section>(() => sectionOf(def));
  return (
    <div className={`right right-bestiary${def?.group === "fantasy" ? " with-speak" : ""}`}>
      <Screen className="s-gallery" title="Specimens" tools={<SectionTabs section={section} onSection={setSection} />}>
        <Gallery section={section} />
      </Screen>
      <Screen className="s-wf" title="Spectrogram" sub="Waterfall of the call">
        <Spectrogram />
      </Screen>
      <Screen className="s-calls" title="Calls">
        {def && <Calls def={def} />}
      </Screen>
      <Screen className="s-cparams" title="Controls">
        {def && <Params def={def} />}
      </Screen>
      {def?.group === "fantasy" && (
        <Screen className="s-speak" title="Speak" sub="English in, its own language out">
          <AlienSpeak key={def.id} def={def} />
        </Screen>
      )}
      <Screen className="s-airway" title="Airway" sub="Each tube unrolled">
        <CreatureTubes />
      </Screen>
      <Screen className="s-keys s-keys-compact" title="Keys" sub="The held voice or a call, at the key's pitch">
        <Keys />
      </Screen>
    </div>
  );
}
