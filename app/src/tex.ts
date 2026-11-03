import type { Lang } from './i18n';
import { LANGUAGES } from './languages';
import { FRAC_WORD, MATH_WORDS, POWER_WORD, SUB_WORD } from './math-words';

// Pure text utilities: no React Native imports, so this module is runnable
// in plain node for testing.

export type MathSpan = { kind: 'text'; text: string } | { kind: 'math'; tex: string };

const MATH_RE = /\\\(([\s\S]+?)\\\)|\\\[([\s\S]+?)\\\]/g;

/** Splits \(...\) and \[...\] spans out of a question/option string. */
export function splitMathSpans(input: string): MathSpan[] {
  const spans: MathSpan[] = [];
  let last = 0;
  MATH_RE.lastIndex = 0;
  for (;;) {
    const match = MATH_RE.exec(input);
    if (match === null) {
      break;
    }
    if (match.index > last) {
      spans.push({ kind: 'text', text: input.slice(last, match.index) });
    }
    spans.push({ kind: 'math', tex: (match[1] ?? match[2]).trim() });
    last = match.index + match[0].length;
  }
  if (last < input.length) {
    spans.push({ kind: 'text', text: input.slice(last) });
  }
  return spans;
}

/** Placeholder width for the font-size heuristic: math is measured as text. */
export function stripMathForMeasure(input: string): string {
  return splitMathSpans(input)
    .map((span) => (span.kind === 'math' ? 'XXXXXX' : span.text))
    .join('');
}

function commandPattern(cmd: string): RegExp {
  // A command ends where letters end: \ne must not match inside \neq.
  // Double backslash: the regex must see a literal backslash first.
  return new RegExp(`\\\\${cmd}(?![a-zA-Z])`, 'g');
}

const MATH_RES = {} as Record<Lang, [RegExp, string][]>;
for (const { code } of LANGUAGES) {
  MATH_RES[code] = Object.entries(MATH_WORDS[code]).map(
    ([cmd, word]) => [commandPattern(cmd), ` ${word} `] as [RegExp, string],
  );
}

function texToSpeech(tex: string, lang: Lang): string {
  const fracWord = FRAC_WORD[lang];
  let s = ` ${tex} `;
  // A limit's subscript is read inline ("limit x approaches 0"), not as "sub".
  s = s.replace(/\\lim(?![a-zA-Z])_/g, '\\lim ');
  // Innermost fractions first, so nesting resolves from the inside out.
  let prev = '';
  while (prev !== s) {
    prev = s;
    s = s.replace(/\\frac\{([^{}]*)\}\{([^{}]*)\}/g, `($1) ${fracWord} ($2)`);
  }
  for (const [re, word] of MATH_RES[lang]) {
    s = s.replace(re, word);
  }
  s = s.replace(/\\[,;:!]/g, ' ');
  s = s
    .replace(/\^/g, ` ${POWER_WORD[lang]} `)
    .replace(/_/g, ` ${SUB_WORD[lang]} `);
  s = s
    .replace(/[{}]/g, ' ')
    .replace(/\\/g, '')
    .replace(/\$/g, '');
  return s.replace(/\s+/g, ' ').trim();
}

/** Question text with math spans converted to speakable words. */
export function stripMathForSpeech(input: string, lang: Lang): string {
  return splitMathSpans(input)
    .map((span) => (span.kind === 'math' ? texToSpeech(span.tex, lang) : span.text))
    .join('')
    .replace(/\s+/g, ' ')
    .trim();
}

export interface SpeechSetup {
  /** BCP-47 code for the TTS voice. */
  tts: string;
  /** Language of the spoken math connector words. */
  math: Lang;
}

export interface NarratorVoiceOption {
  identifier: string;
  name: string;
  language: string;
}

/**
 * Best device voice for a TTS tag, or null when the device has none.
 * Prefers Google voices (consistently good on desktop Chrome) and exact
 * tag matches; avoids robotic engines. Letting the platform choose
 * instead often lands on the worst option (e.g. eSpeak, or an English
 * voice reading Czech).
 */
export function pickNarratorVoice(
  voices: NarratorVoiceOption[],
  ttsTag: string,
): NarratorVoiceOption | null {
  const tag = ttsTag.toLowerCase().replace(/_/g, '-');
  const base = tag.split('-')[0];
  if (base === '') {
    return null;
  }
  let best: NarratorVoiceOption | null = null;
  let bestScore = Number.NEGATIVE_INFINITY;
  for (const voice of voices) {
    const lang = voice.language.toLowerCase().replace(/_/g, '-');
    if (lang.split('-')[0] !== base) {
      continue;
    }
    let score = 0;
    if (lang === tag) {
      score += 4;
    }
    const name = voice.name.toLowerCase();
    if (name.startsWith('google')) {
      score += 2;
    }
    if (name.includes('espeak')) {
      score -= 5;
    }
    if (score > bestScore) {
      best = voice;
      bestScore = score;
    }
  }
  return best;
}

/**
 * Narration follows the subject content language. Every app language is
 * equal here: voice tag and math words both come from the tables, and
 * only unknown codes fall back to English.
 */
export function resolveSpeechLanguage(subjectLang: string, uiLang: Lang): SpeechSetup {
  const code = subjectLang.trim().toLowerCase();
  const match = LANGUAGES.find((entry) => entry.code === code);
  if (match) {
    return { tts: match.tts, math: match.code };
  }
  if (code === '') {
    const ui = LANGUAGES.find((entry) => entry.code === uiLang);
    return { tts: ui?.tts ?? 'en-US', math: uiLang };
  }
  // Unknown content language: let the platform try the raw code, with
  // English math words.
  return { tts: code, math: 'en' };
}
