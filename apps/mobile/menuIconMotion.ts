const CENTER = 16;

function phase(progress: number, start: number, end: number) {
  'worklet';
  const t = Math.max(0, Math.min(1, (progress - start) / (end - start)));
  return t * t * (3 - 2 * t);
}

// Each side of zero is one variant. Interrupted transitions can pass through
// the shared ellipsis without jumping between two different outlines.
export function getMenuIconFrame(signedProgress: number) {
  'worklet';
  const triangle = signedProgress >= 0;
  const progress = Math.abs(signedProgress);
  const move = phase(progress, 0, 0.42);
  const cornerX = 8 - (triangle ? 0.5 * move : 0);
  const cornerY = CENTER - (triangle ? 5 : 3) * move;
  const tipY = CENTER + (triangle ? 7 : 4) * move;

  return {
    triangle,
    points: [
      {x: cornerX, y: cornerY},
      {x: CENTER, y: tipY},
      {x: 32 - cornerX, y: cornerY},
    ],
    edgeProgress: triangle ? phase(progress, 0.42, 0.75) : phase(progress, 0.31, 0.82),
    fillProgress: triangle ? phase(progress, 0.75, 1) : 0,
    dotOpacity: 1 - (triangle ? phase(progress, 0.78, 0.99) : phase(progress, 0.62, 0.92)),
  };
}
