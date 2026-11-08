import { useEffect, useRef, useState } from "react";
import { useStore } from "../store.ts";
import { Chip } from "./Screen.tsx";
import { download, recordClip, type Recording } from "../record.ts";

/** Record / Stop for the big screen: saves a video of its canvas with the sound. */
export function RecordButton() {
  const start = useStore((s) => s.start);
  const name = useStore((s) => (s.mode === "bestiary" ? s.creature : s.mode));
  const rec = useRef<Recording | null>(null);
  const [since, setSince] = useState<number | null>(null);
  const [now, setNow] = useState(0);

  useEffect(() => {
    if (since === null) return;
    const id = setInterval(() => setNow(performance.now()), 250);
    return () => clearInterval(id);
  }, [since]);
  useEffect(() => () => rec.current?.stop(), []);

  const toggle = async (e: React.MouseEvent<HTMLButtonElement>) => {
    if (rec.current) {
      rec.current.stop();
      rec.current = null;
      setSince(null);
      return;
    }
    const canvas = e.currentTarget.closest(".screen")?.querySelector<HTMLCanvasElement>(".screen-body canvas");
    if (!canvas) return;
    await start();
    if (!useStore.getState().started) return;
    const stamp = new Date().toISOString().slice(0, 19).replace(/[T:]/g, "-");
    rec.current = recordClip(canvas, (file, ext) => download(file, `chimera-${name}-${stamp}.${ext}`));
    if (rec.current) setSince(performance.now()), setNow(performance.now());
  };

  const secs = since === null ? 0 : Math.max(0, Math.floor((now - since) / 1000));
  return (
    <Chip toggle={false} tone={since !== null ? "amber" : undefined} on={since !== null} onClick={(e) => void toggle(e)} title="Record a video of this screen with its sound; Stop saves it" aria-label={since !== null ? "Stop recording and save the video" : "Record a video of this screen"}>
      {since !== null ? `● Stop ${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}` : "● Record"}
    </Chip>
  );
}
