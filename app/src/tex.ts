import type { Lang } from './i18n';

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

export type SpeechLang = 'en' | 'sk';

// TeX commands the narrator speaks as words. Digits, letters and symbols
// (+, -, =, parentheses) pass through: the TTS voice reads those in its
// own language.
const MATH_WORDS: Record<SpeechLang, Record<string, string>> = {
  en: {
    lim: 'limit',
    log: 'log',
    ln: 'ln',
    sin: 'sine',
    cos: 'cosine',
    tan: 'tangent',
    exp: 'exp',
    sqrt: 'square root of',
    sum: 'sum',
    int: 'integral',
    infty: 'infinity',
    pi: 'pi',
    alpha: 'alpha',
    beta: 'beta',
    gamma: 'gamma',
    delta: 'delta',
    theta: 'theta',
    lambda: 'lambda',
    mu: 'mu',
    sigma: 'sigma',
    phi: 'phi',
    omega: 'omega',
    to: 'to',
    approx: 'approximately',
    neq: 'not equal to',
    ne: 'not equal to',
    leq: 'less than or equal to',
    le: 'less than or equal to',
    geq: 'greater than or equal to',
    ge: 'greater than or equal to',
    times: 'times',
    cdot: 'times',
    div: 'divided by',
    pm: 'plus or minus',
    left: '',
    right: '',
  },
  sk: {
    lim: 'limita',
    log: 'logaritmus',
    ln: 'ln',
    sin: 'sínus',
    cos: 'kosínus',
    tan: 'tangens',
    exp: 'exponenciála',
    sqrt: 'odmocnina z',
    sum: 'suma',
    int: 'integrál',
    infty: 'nekonečno',
    pi: 'pí',
    alpha: 'alfa',
    beta: 'beta',
    gamma: 'gama',
    delta: 'delta',
    theta: 'téta',
    lambda: 'lambda',
    mu: 'mí',
    sigma: 'sigma',
    phi: 'fí',
    omega: 'omega',
    to: 'sa blíži k',
    approx: 'približne',
    neq: 'nerovná sa',
    ne: 'nerovná sa',
    leq: 'menšie alebo rovné',
    le: 'menšie alebo rovné',
    geq: 'väčšie alebo rovné',
    ge: 'väčšie alebo rovné',
    times: 'krát',
    cdot: 'krát',
    div: 'deleno',
    pm: 'plus alebo mínus',
    left: '',
    right: '',
  },
};

const FRAC_WORD: Record<SpeechLang, string> = { en: 'over', sk: 'lomeno' };
const POWER_WORD: Record<SpeechLang, string> = { en: 'to the power', sk: 'na' };
const SUB_WORD: Record<SpeechLang, string> = { en: 'sub', sk: 's indexom' };

function commandPattern(cmd: string): RegExp {
  // A command ends where letters end: \ne must not match inside \neq.
  return new RegExp(`\\\\${cmd}(?![a-zA-Z])`, 'g');
}

const MATH_RES: Record<SpeechLang, [RegExp, string][]> = {
  en: Object.entries(MATH_WORDS.en).map(([cmd, word]) => [commandPattern(cmd), ` ${word} `]),
  sk: Object.entries(MATH_WORDS.sk).map(([cmd, word]) => [commandPattern(cmd), ` ${word} `]),
};

function texToSpeech(tex: string, lang: SpeechLang): string {
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
export function stripMathForSpeech(input: string, lang: SpeechLang): string {
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
  math: SpeechLang;
}

/** Narration follows the subject content language, not the UI language. */
export function resolveSpeechLanguage(subjectLang: string, uiLang: Lang): SpeechSetup {
  const code = subjectLang.trim().toLowerCase();
  if (code.startsWith('sk')) {
    return { tts: 'sk-SK', math: 'sk' };
  }
  if (code.startsWith('en')) {
    return { tts: 'en-US', math: 'en' };
  }
  if (code === '') {
    return uiLang === 'sk' ? { tts: 'sk-SK', math: 'sk' } : { tts: 'en-US', math: 'en' };
  }
  // Unknown content language: let the platform try the raw code, with
  // math words in the UI language.
  return { tts: subjectLang.trim(), math: uiLang === 'sk' ? 'sk' : 'en' };
}
