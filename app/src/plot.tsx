import React, { useMemo } from 'react';
import Svg, { Circle, G, Line, Path, Text as SvgText } from 'react-native-svg';
import { colors } from './theme';
import { fmt, gridLines, parseExpr, sampleCurve } from './plot-math';

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

    const grid: React.ReactNode[] = [];
    for (const v of gridLines(xMin, xMax, 5)) {
      const px = toX(v);
      grid.push(<Line key={`xg${v}`} x1={px} y1={PAD.top} x2={px} y2={height - PAD.bottom} stroke={GRID_COLOR} strokeWidth={1} />);
      grid.push(
        <SvgText key={`xl${v}`} x={px} y={height - PAD.bottom + 16} fontSize={10} fill={LABEL_COLOR} textAnchor="middle">
          {fmt(v)}
        </SvgText>,
      );
    }
    for (const v of gridLines(yMin, yMax, 4)) {
      const py = toY(v);
      grid.push(<Line key={`yg${v}`} x1={PAD.left} y1={py} x2={width - PAD.right} y2={py} stroke={GRID_COLOR} strokeWidth={1} />);
      grid.push(
        <SvgText key={`yl${v}`} x={PAD.left - 6} y={py + 3.5} fontSize={10} fill={LABEL_COLOR} textAnchor="end">
          {fmt(v)}
        </SvgText>,
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

    const dots = points
      .filter((p) => Number.isFinite(p.x) && Number.isFinite(p.y))
      .map((p, i) => (
        <G key={`p${i}`}>
          <Circle cx={toX(p.x)} cy={toY(p.y)} r={5} fill={POINT_COLOR} stroke="#FFFFFF" strokeWidth={2} />
          {p.label !== '' && (
            <SvgText x={toX(p.x) + 9} y={toY(p.y) - 7} fontSize={11} fontWeight="bold" fill={POINT_COLOR}>
              {p.label}
            </SvgText>
          )}
        </G>
      ));

    const labeled = functions
      .map((fn, i) => ({ fn, i }))
      .filter(({ fn }) => fn.label !== '');
    const legend =
      labeled.length === 0 ? null : (
        <G key="legend">
          {labeled.map(({ fn, i }, row) => (
            <G key={`lg${i}`}>
              <Line
                x1={PAD.left + 6}
                y1={PAD.top + 8 + row * 17}
                x2={PAD.left + 26}
                y2={PAD.top + 8 + row * 17}
                stroke={CURVE_COLORS[i % CURVE_COLORS.length]}
                strokeWidth={3}
                strokeLinecap="round"
              />
              <SvgText x={PAD.left + 32} y={PAD.top + 12 + row * 17} fontSize={11} fill={LABEL_COLOR}>
                {fn.label}
              </SvgText>
            </G>
          ))}
        </G>
      );

    return { height, grid, axes, curves, dots, legend };
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
    </Svg>
  );
}
