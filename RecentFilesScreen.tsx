import React, {useState, useEffect, useCallback, useMemo, useRef} from 'react';
import {
  StyleSheet,
  View,
  Text,
  TouchableOpacity,
  Alert,
  Platform,
  ScrollView,
} from 'react-native';
import Animated, {
  Extrapolation,
  FadeOut,
  interpolate,
  LinearTransition,
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  type SharedValue,
} from 'react-native-reanimated';
import {useNavigation} from '@react-navigation/native';
import {useTranslation} from 'react-i18next';
import {useSafeAreaInsets} from 'react-native-safe-area-context';
import {Gesture, GestureDetector, GestureHandlerRootView} from 'react-native-gesture-handler';
import ReanimatedSwipeable, {type SwipeableMethods} from 'react-native-gesture-handler/ReanimatedSwipeable';
import {scheduleOnRN} from 'react-native-worklets';
import * as Haptics from 'expo-haptics';
import {SymbolView} from 'expo-symbols';

import {MarkdownContext} from './MarkdownContext';
import {getRecentFiles, openRecentEntry, deleteRecentFile, clearAllRecentFiles, type RecentFileEntry, type RecentTarget} from './recentFiles';

const PREVIEW_DELAY_MS = 420;
const PREVIEW_HOLD_MS = 1200;
const REMOVE_ICON_TRAVEL = 88;
const REMOVE_BACKGROUND = '#B42318';
const FULL_SWIPE_FRACTION = 0.8;
const SWIPE_SPRING = {duration: 300, dampingRatio: 0.85};
const supportsFullSwipe = Platform.OS === 'ios' || Platform.OS === 'android';

function RemoveAction({
  progress,
  label,
  reduceMotion,
  onPress,
}: {
  progress: SharedValue<number>;
  label: string;
  reduceMotion: boolean;
  onPress: () => void;
}) {
  const iconStyle = useAnimatedStyle(() => {
    const reveal = progress.get();
    return {
      transform: [
        {translateX: reduceMotion ? 0 : interpolate(reveal, [0, 1], [REMOVE_ICON_TRAVEL, 0], Extrapolation.CLAMP)},
        {scale: reduceMotion ? 1 : interpolate(reveal, [0, 1], [0.7, 1.25], Extrapolation.CLAMP)},
      ],
    };
  });
  const labelStyle = useAnimatedStyle(() => {
    const reveal = progress.get();
    const iconTranslation = interpolate(reveal, [0, 1], [REMOVE_ICON_TRAVEL, 0], Extrapolation.CLAMP);
    const trailingOffset = interpolate(reveal, [0.32, 1], [20, 0], Extrapolation.CLAMP);
    return {
      opacity: interpolate(reveal, [0.32, 0.85], [0, 1], Extrapolation.CLAMP),
      transform: [{translateX: reduceMotion ? 0 : iconTranslation + trailingOffset}],
    };
  });

  return (
    <TouchableOpacity
      testID="recentFileRemove"
      style={styles.removeAction}
      accessibilityRole="button"
      accessibilityLabel={label}
      onPress={onPress}
      activeOpacity={0.7}>
      <Animated.View style={iconStyle}>
        {Platform.OS === 'ios'
          ? <SymbolView name="trash" size={20} tintColor="#fff" />
          : <Text style={styles.removeIcon}>🗑</Text>}
      </Animated.View>
      <Animated.Text style={[styles.removeText, labelStyle]}>{label}</Animated.Text>
    </TouchableOpacity>
  );
}

function RecentFileRow({
  entry,
  removeLabel,
  reduceMotion,
  swipeableRef,
  onOpen,
  onRemove,
  onSwipeStart,
}: {
  entry: RecentFileEntry;
  removeLabel: string;
  reduceMotion: boolean;
  swipeableRef?: React.RefObject<SwipeableMethods | null>;
  onOpen: (entry: RecentFileEntry, target?: RecentTarget) => void;
  onRemove: (id: string) => void;
  onSwipeStart: (id: string) => void;
}) {
  const {theme, backgroundColor} = React.useContext(MarkdownContext);
  const open = useCallback(() => onOpen(entry, entry.folder ? 'folder' : 'file'), [entry, onOpen]);
  const remove = useCallback(() => onRemove(entry.id), [entry.id, onRemove]);
  const beginSwipe = useCallback(() => onSwipeStart(entry.id), [entry.id, onSwipeStart]);
  const opacity = useSharedValue(1);
  const rowWidth = useSharedValue(0);
  const pressedStyle = useAnimatedStyle(() => ({opacity: opacity.get()}));
  const tap = useMemo(() => Gesture.Tap()
    // Cancel before Swipeable's 10-point activation distance so a short drag can't open a file.
    .maxDistance(8)
    .onBegin(() => opacity.set(0.7))
    .onEnd((_event, success) => {
      if (success) {
        scheduleOnRN(open);
      }
    })
    .onFinalize(() => opacity.set(1)), [opacity, open]);
  const fullSwipe = useMemo(() => Gesture.Pan()
    .activeOffsetX(-10)
    .failOffsetY([-18, 18])
    .maxPointers(1)
    .onEnd(({translationX}, success) => {
      const width = rowWidth.get();
      if (success && width > 0 && -translationX >= width * FULL_SWIPE_FRACTION) {
        scheduleOnRN(remove);
      }
    }), [remove, rowWidth]);

  const row = (
    <Animated.View
      exiting={FadeOut.duration(200)}
      layout={LinearTransition.duration(200)}
      onLayout={event => rowWidth.set(event.nativeEvent.layout.width)}>
      <ReanimatedSwipeable
        ref={swipeableRef}
        friction={supportsFullSwipe ? 1.2 : 1.35}
        overshootRight={supportsFullSwipe}
        overshootFriction={1}
        containerStyle={supportsFullSwipe && styles.removeBackground}
        childrenContainerStyle={supportsFullSwipe && {backgroundColor}}
        simultaneousWithExternalGesture={supportsFullSwipe ? fullSwipe : undefined}
        animationOptions={SWIPE_SPRING}
        onSwipeableOpenStartDrag={beginSwipe}
        onSwipeableCloseStartDrag={beginSwipe}
        renderRightActions={progress => (
          <RemoveAction
            progress={progress}
            label={removeLabel}
            reduceMotion={reduceMotion}
            onPress={remove}
          />
        )}>
        <GestureDetector gesture={tap}>
          <Animated.View
            testID={entry.folder ? 'recentFolderRow' : 'recentFileRow'}
            accessible
            accessibilityRole="button"
            accessibilityLabel={entry.folder ? entry.title : `${entry.title}, ${entry.subtitle}`}
            accessibilityActions={[{name: 'activate'}]}
            onAccessibilityAction={event => {
              if (event.nativeEvent.actionName === 'activate') {
                open();
              }
            }}
            onAccessibilityTap={open}
            style={[
              styles.item,
              {backgroundColor, borderBottomColor: entry.folder && entry.subtitle ? 'transparent' : theme.colors.border},
              pressedStyle,
            ]}>
            <Text style={[styles.itemIcon, {color: theme.colors.textMuted}]}>{entry.folder ? '📁' : '📄'}</Text>
            <View style={styles.itemContent}>
              <Text style={[styles.itemTitle, {color: theme.colors.heading}]} numberOfLines={1}>
                {entry.title}
              </Text>
              {!entry.folder && (
                <Text style={[styles.itemSubtitle, {color: theme.colors.textMuted}]} numberOfLines={1}>
                  {entry.subtitle}
                </Text>
              )}
            </View>
          </Animated.View>
        </GestureDetector>
      </ReanimatedSwipeable>
      {entry.folder && !!entry.subtitle && (
        <TouchableOpacity
          testID="recentFolderFileRow"
          accessibilityRole="button"
          style={[styles.folderFile, {backgroundColor, borderBottomColor: theme.colors.border}]}
          onPress={() => onOpen(entry, 'file')}>
          <Text style={[styles.itemSubtitle, {color: theme.colors.link}]} numberOfLines={1}>{entry.subtitle}</Text>
        </TouchableOpacity>
      )}
    </Animated.View>
  );

  return supportsFullSwipe ? <GestureDetector gesture={fullSwipe}>{row}</GestureDetector> : row;
}

export function RecentFilesScreen() {
  const {t} = useTranslation();
  const navigation = useNavigation();
  const insets = useSafeAreaInsets();
  const {
    openFile,
    theme,
    backgroundColor,
  } = React.useContext(MarkdownContext);

  const [recentFiles, setRecentFiles] = useState<RecentFileEntry[]>([]);
  const firstEntryId = recentFiles[0]?.id;
  const reduceMotion = useReducedMotion();
  const firstSwipeable = useRef<SwipeableMethods>(null);
  const previewScheduled = useRef(false);
  const previewOpenTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const previewCloseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setRecentFiles(getRecentFiles());
  }, []);

  const cancelPendingPreview = useCallback(() => {
    if (previewOpenTimer.current !== null) {
      clearTimeout(previewOpenTimer.current);
      previewOpenTimer.current = null;
    }
  }, []);

  const cancelPreview = useCallback(() => {
    cancelPendingPreview();
    if (previewCloseTimer.current !== null) {
      clearTimeout(previewCloseTimer.current);
      previewCloseTimer.current = null;
    }
  }, [cancelPendingPreview]);

  useEffect(() => {
    if (!firstEntryId || reduceMotion || previewScheduled.current) {
      return;
    }

    previewScheduled.current = true;
    previewOpenTimer.current = setTimeout(() => {
      previewOpenTimer.current = null;
      const row = firstSwipeable.current;
      if (row) {
        row.openRight();
        previewCloseTimer.current = setTimeout(() => {
          previewCloseTimer.current = null;
          row.close();
        }, PREVIEW_HOLD_MS);
      }
    }, PREVIEW_DELAY_MS);

    return cancelPreview;
  }, [firstEntryId, reduceMotion, cancelPreview]);

  const handleSwipeStart = useCallback((id: string) => {
    if (previewCloseTimer.current !== null && id !== firstEntryId) {
      firstSwipeable.current?.close();
    }
    cancelPreview();
  }, [firstEntryId, cancelPreview]);

  const handleOpenRecentFile = useCallback(async (entry: RecentFileEntry, target?: RecentTarget) => {
    if (await openRecentEntry(entry, openFile, {target})) {
      navigation.goBack();
    } else if (entry.folder) {
      Alert.alert(t('folder.unavailableTitle'), t('folder.unavailableMessage'));
    } else {
      Alert.alert(t('recentFiles.unavailableTitle'), t('recentFiles.unavailableMessage'));
      setRecentFiles(prev => prev.filter(e => e.id !== entry.id));
      deleteRecentFile(entry.id);
    }
  }, [openFile, navigation, t]);

  const handleDeleteRecentFile = useCallback((id: string) => {
    setRecentFiles(prev => prev.filter(e => e.id !== id));
    deleteRecentFile(id);
    const feedback = Platform.OS === 'android'
      ? Haptics.performAndroidHapticsAsync(Haptics.AndroidHaptics.Confirm)
      : Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium);
    void feedback.catch(() => {});
  }, []);

  const handleClearAll = useCallback(() => {
    setRecentFiles([]);
    clearAllRecentFiles();
  }, []);

  return (
    <GestureHandlerRootView style={styles.container}>
      <View style={[styles.container, {backgroundColor}]} onTouchStart={cancelPendingPreview}>
        <View style={[styles.header, {paddingTop: insets.top + 8}]}>
          <Text style={[styles.title, {color: theme.colors.heading}]}>{t('recentFiles.title')}</Text>
          <View style={styles.headerActions}>
            {recentFiles.length > 0 && (
              <TouchableOpacity onPress={handleClearAll} activeOpacity={0.7}>
                <Text style={[styles.clearAll, {color: theme.colors.link}]}>{t('common.clearAll')}</Text>
              </TouchableOpacity>
            )}
            <TouchableOpacity onPress={() => navigation.goBack()} activeOpacity={0.7}>
              <Text style={[styles.doneButton, {color: theme.colors.link}]}>{t('common.done')}</Text>
            </TouchableOpacity>
          </View>
        </View>
        <ScrollView style={styles.scrollView} contentContainerStyle={styles.scrollContent}>
          {recentFiles.length === 0 && (
            <Text style={[styles.emptyText, {color: theme.colors.textMuted}]}>
              {t('recentFiles.empty')}
            </Text>
          )}
          {recentFiles.map((entry) => (
            <RecentFileRow
              key={entry.id}
              entry={entry}
              removeLabel={t('recentFiles.remove')}
              reduceMotion={reduceMotion}
              swipeableRef={entry.id === firstEntryId ? firstSwipeable : undefined}
              onOpen={handleOpenRecentFile}
              onRemove={handleDeleteRecentFile}
              onSwipeStart={handleSwipeStart}
            />
          ))}
        </ScrollView>
      </View>
    </GestureHandlerRootView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingHorizontal: 16,
    paddingBottom: 12,
  },
  title: {
    fontSize: 22,
    fontWeight: '700',
  },
  headerActions: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 16,
  },
  clearAll: {
    fontSize: 14,
  },
  doneButton: {
    fontSize: 16,
    fontWeight: '600',
  },
  scrollView: {
    flex: 1,
  },
  scrollContent: {
    paddingHorizontal: 16,
    paddingBottom: 32,
  },
  emptyText: {
    fontSize: 15,
    textAlign: 'center',
    marginTop: 48,
  },
  item: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 12,
    paddingHorizontal: 4,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  itemIcon: {
    fontSize: 24,
    marginRight: 12,
  },
  folderFile: {
    minHeight: 44,
    paddingLeft: 44,
    justifyContent: 'center',
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  itemContent: {
    flex: 1,
  },
  itemTitle: {
    fontSize: 16,
    fontWeight: '500',
  },
  itemSubtitle: {
    fontSize: 13,
    marginTop: 2,
  },
  removeBackground: {
    backgroundColor: REMOVE_BACKGROUND,
  },
  removeAction: {
    backgroundColor: REMOVE_BACKGROUND,
    flexDirection: 'row',
    justifyContent: 'center',
    alignItems: 'center',
    minWidth: 132,
    paddingHorizontal: 14,
    gap: 12,
    overflow: 'hidden',
  },
  removeIcon: {
    color: '#fff',
    fontSize: 20,
  },
  removeText: {
    color: '#fff',
    fontWeight: '600',
    fontSize: 14,
  },
});
