/**
 * Record the dialogue stage as one video: every participant's screen is copied, each frame, into a
 * tile of one canvas (titled with the name, captioned with the words being said), and that canvas
 * is recorded with the dialogue's sound.
 */
import { label, labelWidth } from "../ui/render/vector.ts";
import { V } from "../ui/colors.ts";
import { recordClip, type Recording } from "../ui/record.ts";
import type { Transport } from "../show/transport.ts";
import { lineAt, scriptLineAt, type RenderedVoice, type ScriptLine } from "./render.ts";

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

/** `script`: shown beside the heads, the line being said lit (leave it out for the heads alone). */
export function recordStage(panels: StagePanel[], transport: Transport, done: (file: Blob, ext: string) => void, script?: readonly ScriptLine[]): Recording | null {
  const W = 1920, H = 1080;
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext("2d")!;
  const [cols, rows] = grid(panels.length);
  const pad = 16, head = 56, foot = 92;
  const sw = script?.length ? 600 : 0;
  const x0 = sw ? sw + pad : 0;
  const tw = (W - x0 - pad * (cols + 1)) / cols, th = (H - pad * (rows + 1)) / rows;
  const scroller = script?.length ? scriptPanel(ctx, script, pad, pad, sw - pad, H - 2 * pad) : null;

  let raf = 0;
  const draw = () => {
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, W, H);
    const t = transport.time();
    scroller?.(t);
    panels.forEach((p, i) => {
      const x = x0 + pad + (i % cols) * (tw + pad), y = pad + Math.floor(i / cols) * (th + pad);
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
    const w = now.line.words[i].text;
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

/**
 * The script as a scrolling column: each line with its speakers (and mood) above it; the line
 * being said lit in amber with a bar beside it, lines said dimmed, and the view easing to keep
 * the current line a third of the way down.
 */
function scriptPanel(ctx: CanvasRenderingContext2D, lines: readonly ScriptLine[], x: number, y: number, w: number, h: number): (t: number) => void {
  const top = 64, textSize = 19, nameSize = 13, lineGap = 28, blockGap = 22, inset = 26;
  const text = { size: textSize, ls: 0.04 };
  // lay out once: wrap each line's words to the column
  const blocks = lines.map((l) => {
    const rowsOut: string[] = [];
    let row = "";
    for (const word of l.text.split(/\s+/)) {
      const next = row ? `${row} ${word}` : word;
      if (row && labelWidth(ctx, next, text) > w - inset - 18) {
        rowsOut.push(row);
        row = word;
      } else row = next;
    }
    if (row) rowsOut.push(row);
    return { line: l, rows: rowsOut, h: 22 + rowsOut.length * lineGap };
  });
  const offsets: number[] = [];
  let total = 0;
  for (const b of blocks) {
    offsets.push(total);
    total += b.h + blockGap;
  }
  let scroll = 0;
  return (t: number) => {
    const now = scriptLineAt(lines, t);
    // where the view should be: the current line (or the next to come) a third of the way down
    let focus = now;
    if (focus < 0) focus = Math.max(0, lines.findIndex((l) => l.t0 > t));
    const want = Math.max(0, Math.min(Math.max(0, total - (h - top)), offsets[focus] - (h - top) / 3));
    scroll += (want - scroll) * 0.12;

    ctx.strokeStyle = V.blueDim;
    ctx.lineWidth = 1;
    ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
    label(ctx, "SCRIPT", x + 18, y + 34, { size: 22, color: V.blue, ls: 0.18 });
    ctx.save();
    ctx.beginPath();
    ctx.rect(x + 1, y + top - 8, w - 2, h - top + 6);
    ctx.clip();
    blocks.forEach((b, i) => {
      const by = y + top + offsets[i] - scroll;
      if (by > y + h || by + b.h < y + top - 30) return;
      const current = i === now, past = now >= 0 ? i < now : b.line.t1 < t;
      const alpha = current ? 1 : past ? 0.4 : 0.8;
      if (current) {
        ctx.fillStyle = V.amber;
        ctx.globalAlpha = 0.9;
        ctx.fillRect(x + 10, by - 4, 4, b.h);
        ctx.globalAlpha = 1;
      }
      const who = b.line.speakers.join(" + ").toUpperCase();
      label(ctx, who, x + inset, by + 12, { size: nameSize, color: current ? V.amber : V.blueMid, ls: 0.16, alpha });
      if (b.line.mood) label(ctx, `(${b.line.mood})`, x + inset + labelWidth(ctx, who, { size: nameSize, ls: 0.16 }) + 10, by + 12, { size: nameSize, color: V.blueMid, ls: 0.06, alpha: alpha * 0.8 });
      b.rows.forEach((r, k) => label(ctx, r, x + inset, by + 22 + (k + 1) * lineGap - 6, { ...text, color: current ? V.amber : V.blue, alpha }));
    });
    ctx.restore();
  };
}
