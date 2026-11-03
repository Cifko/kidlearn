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
import { splitMathSpans } from './tex';

export { stripMathForMeasure } from './tex';

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

function MathSvg({
  tex,
  fontSize,
  maxWidth,
  maxHeight,
}: {
  tex: string;
  fontSize: number;
  maxWidth?: number;
  maxHeight?: number;
}) {
  const rendered = useMemo(() => texToSvg(tex), [tex]);
  if (rendered === null) {
    return <Text style={{ fontSize }}>{tex}</Text>;
  }
  // MathJax ex units: 1ex is half the font size.
  const naturalWidth = (rendered.widthEx * fontSize) / 2;
  const naturalHeight = (rendered.heightEx * fontSize) / 2;
  let scale = 1;
  if (maxWidth != null && naturalWidth > 0) {
    scale = Math.min(scale, maxWidth / naturalWidth);
  }
  if (maxHeight != null && naturalHeight > 0) {
    scale = Math.min(scale, maxHeight / naturalHeight);
  }
  const width = naturalWidth * scale;
  const height = naturalHeight * scale;
  // Explicit numeric override: the svg root keeps its viewBox, while no
  // ex-unit string ever reaches the native length parser.
  return <SvgXml xml={rendered.svg} override={{ width, height }} />;
}

/**
 * AI-generated formulas are untrusted input: if a rendered SVG ever
 * crashes its renderer (seen once as a WebShape throw on web), degrade
 * to plain text instead of killing the whole lesson screen.
 */
class RenderBoundary extends React.Component<
  { fallback: React.ReactNode; children: React.ReactNode },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }

  render() {
    if (this.state.failed) {
      return this.props.fallback;
    }
    return this.props.children;
  }
}

const FONT_KEYS = ['fontSize', 'color', 'fontWeight', 'fontStyle', 'lineHeight'] as const;

export function MathText({
  text,
  style,
  maxWidth,
  maxHeight,
  center = false,
}: {
  text: string;
  style?: StyleProp<TextStyle>;
  maxWidth?: number;
  maxHeight?: number;
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
    <RenderBoundary fallback={<Text style={style}>{text}</Text>}>
      <View style={[style, rowStyle]}>
        {spans.map((span, index) =>
          span.kind === 'text' ? (
            <Text key={index} style={fontStyle}>
              {span.text}
            </Text>
          ) : (
            <RenderBoundary
              key={`${index}:${span.tex}`}
              fallback={<Text style={fontStyle}>{span.tex}</Text>}
            >
              <MathSvg
                tex={span.tex}
                fontSize={fontSize}
                maxWidth={maxWidth}
                maxHeight={maxHeight}
              />
            </RenderBoundary>
          ),
        )}
      </View>
    </RenderBoundary>
  );
}
