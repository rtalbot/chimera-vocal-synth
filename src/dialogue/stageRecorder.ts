/**
 * Record the dialogue stage as one video: every participant's screen is copied, each frame, into a
 * tile of one canvas (titled with the name, captioned with the words being said), and that canvas
 * is recorded with the dialogue's sound.
 */
import { label, labelWidth } from "../ui/render/vector.ts";
import { V } from "../ui/colors.ts";
import { recordClip, type Recording } from "../ui/record.ts";
import type { Transport } from "../show/transport.ts";
import { lineAt, type RenderedVoice } from "./render.ts";

export interface StagePanel {
  voice: RenderedVoice;
  /** the participant's live screen (its canvas is found inside) */
  el: HTMLElement;
  /** what kind of voice, under the name */
  sub: string;
}

/** tiles across and down for n participants on a 16:9 frame */
function grid(n: number): [number, number] {
  if (n <= 4) return [n, 1];
  if (n <= 8) return [Math.ceil(n / 2), 2];
  return [Math.ceil(n / 3), 3];
}

export function recordStage(panels: StagePanel[], transport: Transport, done: (file: Blob, ext: string) => void): Recording | null {
  const W = 1920, H = 1080;
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext("2d")!;
  const [cols, rows] = grid(panels.length);
  const pad = 16, head = 56, foot = 92;
  const tw = (W - pad * (cols + 1)) / cols, th = (H - pad * (rows + 1)) / rows;

  let raf = 0;
  const draw = () => {
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, W, H);
    const t = transport.time();
    panels.forEach((p, i) => {
      const x = pad + (i % cols) * (tw + pad), y = pad + Math.floor(i / cols) * (th + pad);
      ctx.strokeStyle = V.blueDim;
      ctx.lineWidth = 1;
      ctx.strokeRect(x + 0.5, y + 0.5, tw - 1, th - 1);
      label(ctx, p.voice.member.name.toUpperCase(), x + 18, y + 34, { size: 22, color: V.blue, ls: 0.18 });
      label(ctx, p.sub.toUpperCase(), x + tw - 18, y + 32, { size: 12, color: V.blueMid, ls: 0.16, align: "right" });
      const src = p.el.querySelector("canvas");
      if (src && src.width > 0 && src.height > 0) {
        const bw = tw - 16, bh = th - head - foot;
        const s = Math.min(bw / src.width, bh / src.height);
        const dw = src.width * s, dh = src.height * s;
        ctx.drawImage(src, x + 8 + (bw - dw) / 2, y + head + (bh - dh) / 2, dw, dh);
      }
      caption(ctx, p.voice, t, x + 18, y + th - foot + 30, tw - 36);
    });
    raf = requestAnimationFrame(draw);
  };
  draw();

  const rec = recordClip(canvas, done, transport.outputStream());
  if (!rec) {
    cancelAnimationFrame(raf);
    return null;
  }
  return {
    stop: () => {
      rec.stop();
      // a last frame or two for the recorder before the drawing stops
      setTimeout(() => cancelAnimationFrame(raf), 300);
    },
  };
}

/** the line being said, the word being said lit, wrapped to two lines at most */
function caption(ctx: CanvasRenderingContext2D, v: RenderedVoice, t: number, x: number, y: number, maxW: number): void {
  const now = lineAt(v, t);
  if (!now) return;
  const o = { size: 20, ls: 0.06 };
  const space = labelWidth(ctx, " ", o) + 6;
  let cx = x, cy = y, rowsUsed = 1;
  for (let i = 0; i < now.line.words.length; i++) {
    const w = now.line.words[i].shown;
    const ww = labelWidth(ctx, w, o);
    if (cx > x && cx + ww > x + maxW) {
      if (rowsUsed === 2) break;
      cx = x;
      cy += 30;
      rowsUsed++;
    }
    label(ctx, w, cx, cy, { ...o, color: i === now.word ? V.amber : V.blueMid, alpha: i === now.word ? 1 : 0.75 });
    cx += ww + space;
  }
}
