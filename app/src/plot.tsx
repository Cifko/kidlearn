import React, { useMemo } from 'react';
import Svg, { Circle, G, Line, Path, Text as SvgText } from 'react-native-svg';
import { colors } from './theme';
import {
  fmt,
  gridLines,
  LabelCandidate,
  parseExpr,
  placeLabels,
  sampleCurve,
  truncateLabel,
} from './plot-math';

export interface PlotFunctionSpec {
  expr: string;
  label: string;
}

export interface PlotPointSpec {
  x: number;
  y: number;
  label: string;
}

export interface PlotSpec {
  functions: PlotFunctionSpec[];
  x_min: number;
  x_max: number;
  y_min: number;
  y_max: number;
  points: PlotPointSpec[];
}

const CURVE_COLORS = ['#0984e3', '#d63031', '#00b894', '#e17055'];
const POINT_COLOR = '#d63031';
const GRID_COLOR = '#dfe6e9';
const AXIS_COLOR = '#636e72';
const LABEL_COLOR = '#636e72';
const SAMPLES = 140;
const PAD = { left: 36, right: 12, top: 12, bottom: 26 };

/**
 * Deterministic function plot: axes, grid, up to four curves, marked
 * points. Drawn locally from the spec, no AI call. Returns null when
 * nothing plottable survives (bad ranges, no valid curve).
 */
export function PlotView({ plot, width }: { plot: PlotSpec; width: number }) {
  const drawing = useMemo(() => {
    const { functions, x_min: xMin, x_max: xMax, y_min: yMin, y_max: yMax, points } = plot;
    if (
      !Number.isFinite(xMin) ||
      !Number.isFinite(xMax) ||
      !Number.isFinite(yMin) ||
      !Number.isFinite(yMax) ||
      !(xMin < xMax) ||
      !(yMin < yMax) ||
      functions.length === 0
    ) {
      return null;
    }
    const height = Math.round(width * 0.7);
    const innerW = width - PAD.left - PAD.right;
    const innerH = height - PAD.top - PAD.bottom;
    const toX = (x: number) => PAD.left + ((x - xMin) / (xMax - xMin)) * innerW;
    const toY = (y: number) => PAD.top + (1 - (y - yMin) / (yMax - yMin)) * innerH;

    // Every text element competes in one placement pass, so labels can
    // never pile onto each other no matter how dense the spec is.
    const labelStyles = new Map<
      LabelCandidate,
      { key: string; fontSize: number; fill: string; fontWeight?: 'bold' }
    >();
    const labels: LabelCandidate[] = [];
    const addLabel = (
      cand: LabelCandidate,
      style: { key: string; fontSize: number; fill: string; fontWeight?: 'bold' },
    ) => {
      labels.push(cand);
      labelStyles.set(cand, style);
    };

    const grid: React.ReactNode[] = [];
    for (const v of gridLines(xMin, xMax, 5)) {
      const px = toX(v);
      grid.push(<Line key={`xg${v}`} x1={px} y1={PAD.top} x2={px} y2={height - PAD.bottom} stroke={GRID_COLOR} strokeWidth={1} />);
      addLabel(
        { x: px, y: height - PAD.bottom + 16, text: fmt(v), anchor: 'middle', charWidth: 6, lineHeight: 13 },
        { key: `xl${v}`, fontSize: 10, fill: LABEL_COLOR },
      );
    }
    for (const v of gridLines(yMin, yMax, 4)) {
      const py = toY(v);
      grid.push(<Line key={`yg${v}`} x1={PAD.left} y1={py} x2={width - PAD.right} y2={py} stroke={GRID_COLOR} strokeWidth={1} />);
      addLabel(
        { x: PAD.left - 6, y: py + 3.5, text: fmt(v), anchor: 'end', charWidth: 6, lineHeight: 13 },
        { key: `yl${v}`, fontSize: 10, fill: LABEL_COLOR },
      );
    }

    const axes: React.ReactNode[] = [];
    if (yMin <= 0 && 0 <= yMax) {
      const py = toY(0);
      axes.push(<Line key="xaxis" x1={PAD.left} y1={py} x2={width - PAD.right} y2={py} stroke={AXIS_COLOR} strokeWidth={1.5} />);
    }
    if (xMin <= 0 && 0 <= xMax) {
      const px = toX(0);
      axes.push(<Line key="yaxis" x1={px} y1={PAD.top} x2={px} y2={height - PAD.bottom} stroke={AXIS_COLOR} strokeWidth={1.5} />);
    }

    const curves: React.ReactNode[] = [];
    functions.slice(0, 4).forEach((fn, i) => {
      const evaluate = parseExpr(fn.expr);
      if (evaluate === null) {
        if (__DEV__) {
          console.warn(`Skipping bad plot expression: ${fn.expr}`);
        }
        return;
      }
      let d = '';
      let penDown = false;
      for (const sample of sampleCurve(evaluate, xMin, xMax, yMin, yMax, SAMPLES)) {
        if (sample === null) {
          penDown = false;
          continue;
        }
        const px = toX(sample.x).toFixed(1);
        const py = toY(sample.y).toFixed(1);
        d += penDown ? `L${px},${py}` : `M${px},${py}`;
        penDown = true;
      }
      if (d !== '') {
        curves.push(
          <Path key={`c${i}`} d={d} fill="none" stroke={CURVE_COLORS[i % CURVE_COLORS.length]} strokeWidth={3} strokeLinejoin="round" strokeLinecap="round" />,
        );
      }
    });
    if (curves.length === 0) {
      return null;
    }

    // Dots outside their own axes are model nonsense: skip rather than
    // draw in the label gutter.
    const dots = points
      .filter(
        (p) =>
          Number.isFinite(p.x) &&
          Number.isFinite(p.y) &&
          p.x >= xMin &&
          p.x <= xMax &&
          p.y >= yMin &&
          p.y <= yMax,
      )
      .map((p, i) => {
        if (p.label !== '') {
          addLabel(
            {
              x: toX(p.x) + 9,
              y: toY(p.y) - 7,
              text: truncateLabel(p.label),
              anchor: 'start',
              charWidth: 6.8,
              lineHeight: 14,
            },
            { key: `pl${i}`, fontSize: 11, fill: POINT_COLOR, fontWeight: 'bold' },
          );
        }
        return (
          <Circle
            key={`p${i}`}
            cx={toX(p.x)}
            cy={toY(p.y)}
            r={5}
            fill={POINT_COLOR}
            stroke="#FFFFFF"
            strokeWidth={2}
          />
        );
      });

    const legendEntries = functions
      .map((fn, i) => ({ fn, i }))
      .filter(({ fn }) => fn.label !== '')
      .map(({ fn, i }, row) => {
        const cand: LabelCandidate = {
          x: PAD.left + 32,
          y: PAD.top + 12 + row * 17,
          text: truncateLabel(fn.label, 28),
          anchor: 'start',
          charWidth: 6.8,
          lineHeight: 14,
        };
        addLabel(cand, { key: `lg${i}`, fontSize: 11, fill: LABEL_COLOR });
        return { cand, i, row };
      });

    const kept = placeLabels(labels);
    const keptSet = new Set(kept);
    const legend =
      legendEntries.length === 0 ? null : (
        <G key="legend">
          {legendEntries
            .filter((entry) => keptSet.has(entry.cand))
            .map(({ i, row }) => (
              <Line
                key={`lgl${i}`}
                x1={PAD.left + 6}
                y1={PAD.top + 8 + row * 17}
                x2={PAD.left + 26}
                y2={PAD.top + 8 + row * 17}
                stroke={CURVE_COLORS[i % CURVE_COLORS.length]}
                strokeWidth={3}
                strokeLinecap="round"
              />
            ))}
        </G>
      );

    const texts = kept.map((cand) => {
      const style = labelStyles.get(cand);
      if (!style) {
        return null;
      }
      return (
        <SvgText
          key={style.key}
          x={cand.x}
          y={cand.y}
          fontSize={style.fontSize}
          fill={style.fill}
          fontWeight={style.fontWeight}
          textAnchor={cand.anchor}
        >
          {cand.text}
        </SvgText>
      );
    });

    return { height, grid, axes, curves, dots, legend, texts };
  }, [plot, width]);

  if (drawing === null) {
    return null;
  }
  return (
    <Svg
      width={width}
      height={drawing.height}
      viewBox={`0 0 ${width} ${drawing.height}`}
      style={{
        backgroundColor: '#FFFFFF',
        borderRadius: 16,
        borderWidth: 3,
        borderColor: colors.border,
        alignSelf: 'center',
        marginBottom: 16,
      }}
    >
      {drawing.grid}
      {drawing.axes}
      {drawing.curves}
      {drawing.dots}
      {drawing.legend}
      {drawing.texts}
    </Svg>
  );
}
