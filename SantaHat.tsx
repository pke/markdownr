import React from 'react';
import Svg, {Path, Circle, Ellipse} from 'react-native-svg';

interface SantaHatProps {
  size?: number;
}

export function SantaHat({size = 24}: SantaHatProps) {
  return (
    <Svg width={size} height={size} viewBox="0 0 64 64">
      {/* White fur trim — full width */}
      <Ellipse cx="32" cy="56" rx="32" ry="8" fill="#ffffff" />
      {/* Red hat body — fills from edges to tip */}
      <Path
        d="M0 56 C4 30 16 14 32 4 C48 14 60 30 64 56 Z"
        fill="#cc0000"
      />
      {/* Darker red shadow on right half */}
      <Path
        d="M32 4 C48 14 60 30 64 56 L32 56 Z"
        fill="#aa0000"
        opacity={0.4}
      />
      {/* White pompom at tip */}
      <Circle cx="32" cy="4" r="4" fill="#ffffff" />
    </Svg>
  );
}
