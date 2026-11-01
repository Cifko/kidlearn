import { Parser } from 'expr-eval';

/** Compiled `y = f(x)` evaluator, or null when the expression is invalid. */
export function parseExpr(expr: string): ((x: number) => number) | null {
  try {
    const parsed = new Parser().parse(expr);
    // Unknown identifiers parse fine but throw on evaluate: probe once so
    // they reject the curve instead of failing per sample.
    parsed.evaluate({ x: 0 });
    return (x: number) => {
      const y = parsed.evaluate({ x });
      return typeof y === 'number' ? y : NaN;
    };
  } catch {
    return null;
  }
}

export type Sample = { x: number; y: number } | null;

/**
 * Samples a curve; null entries mark pen lifts (gaps, infinities, jumps
 * taller than the plot, which signal discontinuities, not lines).
 */
export function sampleCurve(
  fn: (x: number) => number,
  xMin: number,
  xMax: number,
  yMin: number,
  yMax: number,
  count: number,
): Sample[] {
  const span = yMax - yMin;
  const samples: Sample[] = [];
  let prevY: number | null = null;
  for (let i = 0; i <= count; i++) {
    const x = xMin + ((xMax - xMin) * i) / count;
    let y: number;
    try {
      y = fn(x);
    } catch {
      y = NaN;
    }
    if (!Number.isFinite(y)) {
      samples.push(null);
      prevY = null;
      continue;
    }
    const clamped = Math.min(yMax + span / 2, Math.max(yMin - span / 2, y));
    if (prevY !== null && Math.abs(clamped - prevY) > span * 1.5) {
      samples.push(null);
    }
    samples.push({ x, y: clamped });
    prevY = clamped;
  }
  return samples;
}

/** Round grid step near raw (1/2/5 series). */
export function niceStep(raw: number): number {
  const mag = Math.pow(10, Math.floor(Math.log10(raw)));
  const norm = raw / mag;
  return (norm >= 5 ? 5 : norm >= 2 ? 2 : 1) * mag;
}

/** Grid line positions across [min, max]; never outside the range. */
export function gridLines(min: number, max: number, targetCount: number): number[] {
  const step = niceStep((max - min) / targetCount);
  const lines: number[] = [];
  const eps = 1e-9 * Math.max(1, Math.abs(max));
  for (let v = Math.ceil(min / step - 1e-9) * step; v <= max + eps; v += step) {
    lines.push(v);
  }
  return lines;
}

/** Short axis label: integers plain, else up to 4 significant digits. */
export function fmt(n: number): string {
  const v = Object.is(n, -0) ? 0 : n;
  return String(parseFloat(v.toPrecision(4)));
}

export interface LabelCandidate {
  x: number;
  y: number;
  text: string;
  anchor: 'start' | 'middle' | 'end';
  /** Pixels per character and per line for the label's font size. */
  charWidth: number;
  lineHeight: number;
}

interface Box {
  x0: number;
  x1: number;
  y0: number;
  y1: number;
}

function boxesOverlap(a: Box, b: Box): boolean {
  return a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0;
}

/**
 * Greedy label placement: walks candidates top to bottom, keeping a label
 * only when its box overlaps nothing kept so far. Overlapping labels come
 * out as a legible subset instead of an unreadable pile.
 */
export function placeLabels(cands: LabelCandidate[]): LabelCandidate[] {
  const placed: Box[] = [];
  const kept: LabelCandidate[] = [];
  const sorted = [...cands].sort((a, b) => a.y - b.y || a.x - b.x);
  for (const c of sorted) {
    const w = c.text.length * c.charWidth;
    const x0 = c.anchor === 'end' ? c.x - w : c.anchor === 'middle' ? c.x - w / 2 : c.x;
    const box: Box = { x0, x1: x0 + w, y0: c.y - c.lineHeight * 0.8, y1: c.y + c.lineHeight * 0.2 };
    if (placed.every((p) => !boxesOverlap(box, p))) {
      placed.push(box);
      kept.push(c);
    }
  }
  return kept;
}

/** A label too long to ever fit is cut rather than spilling everywhere. */
export function truncateLabel(text: string, max = 20): string {
  return text.length <= max ? text : `${text.slice(0, max - 1)}…`;
}
