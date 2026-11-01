import React, { useMemo } from 'react';
import { StyleProp, StyleSheet, Text, TextStyle, View, ViewStyle } from 'react-native';
import { SvgXml } from 'react-native-svg';
import { mathjax } from 'mathjax-full/js/mathjax.js';
import { TeX } from 'mathjax-full/js/input/tex.js';
import { SVG } from 'mathjax-full/js/output/svg.js';
import { liteAdaptor } from 'mathjax-full/js/adaptors/liteAdaptor.js';
import { RegisterHTMLHandler } from 'mathjax-full/js/handlers/html.js';
import 'mathjax-full/js/input/tex/ams/AmsConfiguration.js';
import 'mathjax-full/js/input/tex/newcommand/NewcommandConfiguration.js';

// MathJax without a DOM: TeX in, self-contained SVG out (glyph paths are
// inlined, so rendering works offline). One code path for web and native.
const adaptor = liteAdaptor();
RegisterHTMLHandler(adaptor);
const tex = new TeX({ packages: ['base', 'ams', 'newcommand'] });
const svgOutput = new SVG({ fontCache: 'local' });
const mjDoc = mathjax.document('', { InputJax: tex, OutputJax: svgOutput });

interface RenderedMath {
  svg: string;
  widthEx: number;
  heightEx: number;
}

const renderCache = new Map<string, RenderedMath | null>();

/** Converts one TeX formula to SVG. Returns null for invalid TeX. */
function texToSvg(source: string): RenderedMath | null {
  const cached = renderCache.get(source);
  if (cached !== undefined) {
    return cached;
  }
  let result: RenderedMath | null = null;
  try {
    const node = mjDoc.convert(source, { display: false });
    const wrapped = adaptor.outerHTML(node);
    // convert() returns a <mjx-container> wrapper: SvgXml needs the inner
    // <svg> as root. As a nested svg, its ex-unit width/height gets
    // truncated to a few points on native (parseInt("4.023ex") === 4),
    // clipping the formula to a sliver. Web only worked by CSS luck.
    const svg = /<svg[\s\S]*<\/svg>/.exec(wrapped)?.[0] ?? wrapped;
    // MathJax renders errors as red text instead of throwing.
    if (!svg.includes('merror')) {
      const width = /width="([\d.]+)ex"/.exec(svg);
      const height = /height="([\d.]+)ex"/.exec(svg);
      if (width && height) {
        result = {
          svg,
          widthEx: parseFloat(width[1]),
          heightEx: parseFloat(height[1]),
        };
      }
    }
  } catch {
    result = null;
  }
  renderCache.set(source, result);
  return result;
}

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

function MathSvg({ tex, fontSize, maxWidth }: { tex: string; fontSize: number; maxWidth?: number }) {
  const rendered = useMemo(() => texToSvg(tex), [tex]);
  if (rendered === null) {
    return <Text style={{ fontSize }}>{tex}</Text>;
  }
  // MathJax ex units: 1ex is half the font size.
  const scale = maxWidth == null ? 1 : Math.min(1, maxWidth / ((rendered.widthEx * fontSize) / 2));
  const width = ((rendered.widthEx * fontSize) / 2) * scale;
  const height = ((rendered.heightEx * fontSize) / 2) * scale;
  // Explicit numeric override: the svg root keeps its viewBox, while no
  // ex-unit string ever reaches the native length parser.
  return <SvgXml xml={rendered.svg} override={{ width, height }} />;
}

const FONT_KEYS = ['fontSize', 'color', 'fontWeight', 'fontStyle', 'lineHeight'] as const;

export function MathText({
  text,
  style,
  maxWidth,
  center = false,
}: {
  text: string;
  style?: StyleProp<TextStyle>;
  maxWidth?: number;
  center?: boolean;
}) {
  const spans = useMemo(() => splitMathSpans(text), [text]);
  if (spans.every((span) => span.kind === 'text')) {
    return <Text style={style}>{text}</Text>;
  }
  const flat = StyleSheet.flatten(style) ?? {};
  const fontStyle: TextStyle = {};
  for (const key of FONT_KEYS) {
    const value = flat[key];
    if (value !== undefined) {
      (fontStyle as Record<string, unknown>)[key] = value;
    }
  }
  const fontSize = typeof flat.fontSize === 'number' ? flat.fontSize : 16;
  const rowStyle: ViewStyle = {
    flexDirection: 'row',
    flexWrap: 'wrap',
    alignItems: 'center',
    justifyContent: center ? 'center' : 'flex-start',
  };
  return (
    <View style={[style, rowStyle]}>
      {spans.map((span, index) =>
        span.kind === 'text' ? (
          <Text key={index} style={fontStyle}>
            {span.text}
          </Text>
        ) : (
          <MathSvg key={index} tex={span.tex} fontSize={fontSize} maxWidth={maxWidth} />
        ),
      )}
    </View>
  );
}
