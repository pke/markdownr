import React, {createContext, useContext, useEffect, useMemo} from 'react';
import {useWindowDimensions, StyleSheet} from 'react-native';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withRepeat,
  withTiming,
  withDelay,
  Easing,
} from 'react-native-reanimated';
import type {ParticleOverlayConfig} from './themes';

interface Particle {
  char: string;
  size: number;
  x: number;
  delay: number;
  fallDuration: number;
  swayAmount: number;
  swayDuration: number;
  rotation: number;
  rotationDuration: number;
  opacity: number;
  behind: boolean;
}

function generateParticles(config: ParticleOverlayConfig, screenWidth: number): Particle[] {
  const {
    particles, count, size, duration, sway, swayDivisor,
    rotation, rotationDivisor, opacity, behindFraction,
  } = config;

  return Array.from({length: count}, () => {
    const fallDuration = duration[0] + Math.random() * (duration[1] - duration[0]);
    return {
      char: particles[Math.floor(Math.random() * particles.length)],
      size: size[0] + Math.random() * (size[1] - size[0]),
      x: Math.random() * screenWidth,
      delay: Math.random() * 10000,
      fallDuration,
      swayAmount: sway[0] + Math.random() * (sway[1] - sway[0]),
      swayDuration: fallDuration / swayDivisor,
      rotation: rotation[0] + Math.random() * (rotation[1] - rotation[0]),
      rotationDuration: fallDuration / rotationDivisor,
      opacity: opacity ? opacity[0] + Math.random() * (opacity[1] - opacity[0]) : 1,
      behind: behindFraction ? Math.random() < behindFraction : false,
    };
  });
}

function AnimatedParticle({
  particle,
  screenHeight,
  color,
  topOffset,
}: {
  particle: Particle;
  screenHeight: number;
  color?: string;
  topOffset: number;
}) {
  const translateY = useSharedValue(-topOffset);
  const translateX = useSharedValue(0);
  const rotate = useSharedValue(0);

  useEffect(() => {
    translateY.value = withDelay(
      particle.delay,
      withRepeat(
        withTiming(screenHeight + topOffset, {
          duration: particle.fallDuration,
          easing: Easing.linear,
        }),
        -1,
        false,
      ),
    );
    translateX.value = withDelay(
      particle.delay,
      withRepeat(
        withTiming(particle.swayAmount, {
          duration: particle.swayDuration,
          easing: Easing.inOut(Easing.sin),
        }),
        -1,
        true,
      ),
    );
    rotate.value = withDelay(
      particle.delay,
      withRepeat(
        withTiming(particle.rotation, {
          duration: particle.rotationDuration,
          easing: Easing.inOut(Easing.sin),
        }),
        -1,
        true,
      ),
    );
  }, [translateY, translateX, rotate, particle, screenHeight, topOffset]);

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [
      {translateY: translateY.value},
      {translateX: translateX.value},
      {rotate: `${rotate.value}deg`},
    ],
  }));

  return (
    <Animated.Text
      style={[
        styles.particle,
        {
          top: -topOffset,
          left: particle.x,
          fontSize: particle.size,
          opacity: particle.opacity,
          ...(color ? {color} : {}),
        },
        animatedStyle,
      ]}
      pointerEvents="none">
      {particle.char}
    </Animated.Text>
  );
}

const ParticleContext = createContext<{particles: Particle[]; topOffset: number}>({particles: [], topOffset: 30});

export function ParticleOverlayProvider({
  config,
  children,
}: {
  config: ParticleOverlayConfig;
  children: React.ReactNode;
}) {
  const {width} = useWindowDimensions();
  const particles = useMemo(() => generateParticles(config, width), [width, config]);
  const topOffset = config.size[1] + 10;

  return (
    <ParticleContext.Provider value={{particles, topOffset}}>
      {children}
    </ParticleContext.Provider>
  );
}

export function ParticleOverlayBackground({color}: {color?: string}) {
  const {height} = useWindowDimensions();
  const {particles, topOffset} = useContext(ParticleContext);
  const behind = useMemo(() => particles.filter(p => p.behind), [particles]);

  return (
    <>
      {behind.map((p, i) => (
        <AnimatedParticle key={i} particle={p} screenHeight={height} color={color} topOffset={topOffset} />
      ))}
    </>
  );
}

export function ParticleOverlayForeground({color}: {color?: string}) {
  const {height} = useWindowDimensions();
  const {particles, topOffset} = useContext(ParticleContext);
  const front = useMemo(() => particles.filter(p => !p.behind), [particles]);

  return (
    <>
      {front.map((p, i) => (
        <AnimatedParticle key={i} particle={p} screenHeight={height} color={color} topOffset={topOffset} />
      ))}
    </>
  );
}

const styles = StyleSheet.create({
  particle: {
    position: 'absolute',
  },
});
