import React, { ReactNode } from 'react';
import { StyleSheet } from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withTiming,
} from 'react-native-reanimated';

interface ZoomableViewProps {
  children: ReactNode;
  minScale?: number;
  maxScale?: number;
}

export function ZoomableView({
  children,
  minScale = 1,
  maxScale = 3,
}: ZoomableViewProps) {
  const scale = useSharedValue(1);
  const savedScale = useSharedValue(1);
  const focalX = useSharedValue(0);
  const focalY = useSharedValue(0);
  const translateX = useSharedValue(0);
  const translateY = useSharedValue(0);
  const savedTranslateX = useSharedValue(0);
  const savedTranslateY = useSharedValue(0);


  const pinchGesture = Gesture.Pinch()
    .onStart((e) => {
      savedScale.value = scale.value;
      focalX.value = e.focalX;
      focalY.value = e.focalY;
    })
    .onUpdate((e) => {
      const newScale = Math.min(Math.max(savedScale.value * e.scale, minScale), maxScale);
      scale.value = newScale;
    })
    .onEnd(() => {
      if (scale.value < minScale) {
        scale.value = withTiming(minScale);
      }
      savedScale.value = scale.value;
    });

  const panGesture = Gesture.Pan()
    .minPointers(2)
    .onStart(() => {
      savedTranslateX.value = translateX.value;
      savedTranslateY.value = translateY.value;
    })
    .onUpdate((e) => {
      if (scale.value > 1) {
        translateX.value = savedTranslateX.value + e.translationX;
        translateY.value = savedTranslateY.value + e.translationY;
      }
    })
    .onEnd(() => {
      // Reset translation when scale is back to 1
      if (scale.value <= 1) {
        translateX.value = withTiming(0);
        translateY.value = withTiming(0);
      }
    });

  const doubleTapGesture = Gesture.Tap()
    .numberOfTaps(2)
    .maxDelay(250)
    .onEnd(() => {
      if (scale.value > 1) {
        scale.value = withTiming(1);
        translateX.value = withTiming(0);
        translateY.value = withTiming(0);
        savedScale.value = 1;
      } else {
        scale.value = withTiming(2);
        savedScale.value = 2;
      }
    });

  const pinchPanGesture = Gesture.Simultaneous(pinchGesture, panGesture);
  const composedGesture = Gesture.Exclusive(doubleTapGesture, pinchPanGesture);

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [
      { translateX: translateX.value },
      { translateY: translateY.value },
      { scale: scale.value },
    ],
  }));

  return (
    <GestureDetector gesture={composedGesture}>
      <Animated.View style={[styles.container, animatedStyle]}>
        {children}
      </Animated.View>
    </GestureDetector>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
});
