import React from 'react';
import { Circle, Ellipse, Path, Svg, Text as SvgText } from 'react-native-svg';

const BODY = '#9C6B30';
const DARK = '#7C5222';
const BELLY = '#EBCB90';
const BEAK = '#E8912D';
const INK = '#2B2B2B';

/** The narrator: an owl, awake when reading aloud, asleep when muted. */
export function Owl({ size = 56, awake = true }: { size?: number; awake?: boolean }) {
  return (
    <Svg width={size} height={size} viewBox="0 0 64 64">
      {/* Ear tufts */}
      <Path d="M14 16 L20 4 L27 15 Z" fill={DARK} />
      <Path d="M37 15 L44 4 L50 16 Z" fill={DARK} />
      {/* Wings behind the body */}
      <Ellipse cx={10} cy={38} rx={6} ry={13} fill={DARK} />
      <Ellipse cx={54} cy={38} rx={6} ry={13} fill={DARK} />
      {/* Body and belly */}
      <Ellipse cx={32} cy={37} rx={22} ry={23} fill={BODY} />
      <Ellipse cx={32} cy={45} rx={13} ry={14} fill={BELLY} />
      {awake ? (
        <>
          {/* Open eyes */}
          <Circle cx={23} cy={27} r={9} fill="#FFFFFF" />
          <Circle cx={41} cy={27} r={9} fill="#FFFFFF" />
          <Circle cx={23} cy={28} r={4.5} fill={INK} />
          <Circle cx={41} cy={28} r={4.5} fill={INK} />
          <Circle cx={24.5} cy={26.5} r={1.5} fill="#FFFFFF" />
          <Circle cx={42.5} cy={26.5} r={1.5} fill="#FFFFFF" />
          {/* Beak */}
          <Path d="M28 35 L36 35 L32 41 Z" fill={BEAK} />
        </>
      ) : (
        <>
          {/* Closed eyes */}
          <Path d="M15 27 Q23 33 31 27" stroke={INK} strokeWidth={2.5} fill="none" strokeLinecap="round" />
          <Path d="M33 27 Q41 33 49 27" stroke={INK} strokeWidth={2.5} fill="none" strokeLinecap="round" />
          <Path d="M28 35 L36 35 L32 39 Z" fill={BEAK} />
          <SvgText x={50} y={14} fontSize={11} fontWeight="900" fill={INK}>
            Z
          </SvgText>
          <SvgText x={55} y={8} fontSize={8} fontWeight="900" fill={INK}>
            z
          </SvgText>
        </>
      )}
      {/* Feet */}
      <Path d="M24 59 L24 62 M28 59 L28 62" stroke={BEAK} strokeWidth={2.5} strokeLinecap="round" />
      <Path d="M36 59 L36 62 M40 59 L40 62" stroke={BEAK} strokeWidth={2.5} strokeLinecap="round" />
    </Svg>
  );
}
