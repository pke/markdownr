import React, {useEffect, useRef} from 'react';
import Animated, {
  Easing,
  ReduceMotion,
  useAnimatedProps,
  useDerivedValue,
  useSharedValue,
  withTiming,
  type DerivedValue,
  type SharedValue,
} from 'react-native-reanimated';
import Svg, {Circle, Path, type CircleProps, type PathProps} from 'react-native-svg';
import {getMenuIconFrame} from './menuIconMotion';

const AnimatedCircle = Animated.createAnimatedComponent(Circle);
const AnimatedPath = Animated.createAnimatedComponent(Path);
type IconFrame = ReturnType<typeof getMenuIconFrame>;
type Point = IconFrame['points'][number];

export function useMenuIconProgress(isOpen: boolean) {
  const progress = useSharedValue(0);
  const nextVariant = useRef(1);

  useEffect(() => {
    const target = isOpen ? nextVariant.current : 0;
    if (isOpen) nextVariant.current = -target;

    progress.set(withTiming(target, {
      duration: target === 1 ? 290 : target === -1 ? 220 : 200,
      easing: Easing.linear,
      reduceMotion: ReduceMotion.System,
    }));
  }, [isOpen, progress]);

  return progress;
}

function segment(from: Point, to: Point, progress: number) {
  'worklet';
  const x = from.x + (to.x - from.x) * progress;
  const y = from.y + (to.y - from.y) * progress;
  return `M ${from.x} ${from.y} L ${x} ${y}`;
}

function MenuDot({frame, index, color}: {
  frame: DerivedValue<IconFrame>;
  index: number;
  color: string;
}) {
  const animatedProps = useAnimatedProps<CircleProps>(() => {
    const {points, dotOpacity} = frame.get();
    return {cx: points[index].x, cy: points[index].y, opacity: dotOpacity};
  });

  return <AnimatedCircle r={1.8} fill={color} animatedProps={animatedProps} />;
}

export function MenuFabIcon({progress, color}: {
  progress: SharedValue<number>;
  color: string;
}) {
  const frame = useDerivedValue(() => getMenuIconFrame(progress.get()));

  const edges = useAnimatedProps<PathProps>(() => {
    const {points: [left, tip, right], triangle, edgeProgress} = frame.get();
    let d = segment(left, tip, edgeProgress) + ' ' + segment(right, tip, edgeProgress);
    if (triangle) d += ' ' + segment(left, right, edgeProgress);
    return {d, opacity: edgeProgress > 0 ? 1 : 0};
  });

  const fill = useAnimatedProps<PathProps>(() => {
    const {fillProgress} = frame.get();
    const y = 11 + 12 * fillProgress;
    const halfWidth = 8.5 * (1 - fillProgress);
    const d = `M 7.5 11 H 24.5 L ${16 + halfWidth} ${y} H ${16 - halfWidth} Z`;
    return {d, opacity: fillProgress > 0 ? 1 : 0};
  });

  return (
    <Svg width={32} height={32} viewBox="0 0 32 32" accessible={false} pointerEvents="none">
      <AnimatedPath fill="none" stroke={color} strokeWidth={2}
        strokeLinecap="round" animatedProps={edges} />
      <AnimatedPath fill={color} animatedProps={fill} />
      {[0, 1, 2].map((index) => (
        <MenuDot key={index} frame={frame} index={index} color={color} />
      ))}
    </Svg>
  );
}
