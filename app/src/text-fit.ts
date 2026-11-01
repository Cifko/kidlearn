// Option buttons are a fixed 150px square with 8px horizontal padding.
// Answers shrink until they fit inside, so the student always sees the
// full text: no ellipses, no clipping. Estimates are deliberately
// conservative (wide chars, tall lines), so real rendering fits with
// room to spare. Plain arithmetic on purpose: adjustsFontSizeToFit is
// silently ignored on web, so a prop-based fix truncates instead of
// shrinking there.
export const OPTION_INNER = 134;

const MIN_OPTION_FONT = 5;
const MAX_OPTION_FONT = 26;
const CHAR_WIDTH = 0.65;
const LINE_HEIGHT = 1.35;

/** Greedy word-wrap line count, the way React Native packs words. */
export function estimatedLines(text: string, fontSize: number): number {
  const capacity = Math.max(1, Math.floor(OPTION_INNER / (CHAR_WIDTH * fontSize)));
  let lines = 1;
  let used = 0;
  for (const word of text.split(/\s+/)) {
    if (word.length === 0) {
      continue;
    }
    const need = used === 0 ? word.length : used + 1 + word.length;
    if (need <= capacity) {
      used = need;
    } else {
      lines += 1;
      used = word.length;
    }
  }
  return lines;
}

export function optionFontSize(text: string): number {
  const longest = Math.max(1, ...text.split(/\s+/).map((word) => word.length));
  const widthCapped = Math.floor(132 / (0.6 * longest));
  const totalCapped =
    text.length <= 8 ? 26 : text.length <= 16 ? 22 : text.length <= 30 ? 18 : 14;
  let size = Math.min(MAX_OPTION_FONT, widthCapped, totalCapped);
  while (
    size > MIN_OPTION_FONT &&
    estimatedLines(text, size) * size * LINE_HEIGHT > OPTION_INNER
  ) {
    size -= 1;
  }
  return Math.max(MIN_OPTION_FONT, size);
}
