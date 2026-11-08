/**
 * Record a clip of the big screen: its canvas (the creature, or the vocal tract) and what you hear,
 * into one video file, downloaded when you stop. MP4 where the browser records it, else WebM.
 */
import { engine } from "../audio/AudioEngine.ts";

// avc1, not avc3: X (and other sites) refuse avc3 ("incompatible video codecs"). avc1 can't change
// size mid-recording, so the canvas recorded must keep its size: the dialogue's composite is fixed.
const TYPES = ["video/mp4;codecs=avc1,mp4a.40.2", "video/mp4", "video/webm;codecs=vp9,opus", "video/webm"];

export interface Recording {
  stop(): void;
}

/** Start recording `canvas` with a sound (the live engine's output unless given); `done` gets the
 *  file when it stops. */
export function recordClip(canvas: HTMLCanvasElement, done: (file: Blob, ext: string) => void, audio = engine.outputStream()): Recording | null {
  if (!audio || typeof MediaRecorder === "undefined") return null;
  const video = canvas.captureStream(60);
  const stream = new MediaStream([...video.getVideoTracks(), ...audio.stream.getAudioTracks()]);
  const mimeType = TYPES.find((t) => MediaRecorder.isTypeSupported(t));
  const rec = new MediaRecorder(stream, { mimeType, videoBitsPerSecond: 8_000_000, audioBitsPerSecond: 192_000 });
  const chunks: Blob[] = [];
  rec.ondataavailable = (e) => e.data.size && chunks.push(e.data);
  rec.onstop = () => {
    audio.stop();
    for (const t of stream.getTracks()) t.stop();
    const type = rec.mimeType || mimeType || "video/webm";
    done(new Blob(chunks, { type }), type.includes("mp4") ? "mp4" : "webm");
  };
  rec.start(250);
  return { stop: () => rec.state !== "inactive" && rec.stop() };
}

/** Save a blob as a download. */
export function download(file: Blob, name: string): void {
  const url = URL.createObjectURL(file);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}
