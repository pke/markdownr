import React, {useCallback, useContext} from 'react';
import {StyleSheet, Text, TouchableOpacity, View} from 'react-native';
import Animated, {FadeInUp, FadeOutDown} from 'react-native-reanimated';
import {useTranslation} from 'react-i18next';

import {MarkdownContext} from './MarkdownContext';

type FileChangedBannerProps = {
  bottomInset?: number;
  /** Current scroll position as a fraction of content height, for restore. */
  getScrollPercent?: () => number | null;
};

/**
 * Offered when the displayed file was modified by an external app (detected on
 * app focus — see fileChangeDetection.ts). Visual pattern mirrors the
 * ThemeSuggestion banner.
 */
export function FileChangedBanner({bottomInset, getScrollPercent}: FileChangedBannerProps) {
  const {t} = useTranslation();
  const {
    externalChangeDetected,
    reloadCurrentFile,
    dismissExternalChange,
    theme,
    backgroundColor,
    setScrollToPercent,
  } = useContext(MarkdownContext);

  const handleReload = useCallback(async () => {
    const percent = getScrollPercent?.() ?? null;
    await reloadCurrentFile();
    if (percent !== null) setScrollToPercent(percent);
  }, [getScrollPercent, reloadCurrentFile, setScrollToPercent]);

  if (!externalChangeDetected) return null;

  return (
    <Animated.View
      entering={FadeInUp.duration(300)}
      exiting={FadeOutDown.duration(200)}
      testID="fileChangedBanner"
      style={[
        styles.container,
        {bottom: bottomInset, right: 20, backgroundColor, borderColor: theme.colors.border},
      ]}>
      <Text style={[styles.text, {color: theme.colors.text}]}>
        {t('fileChanged.message')}
      </Text>
      <View style={styles.actions}>
        <TouchableOpacity
          onPress={dismissExternalChange}
          style={[styles.dismissButton, {borderColor: theme.colors.border}]}>
          <Text style={[styles.dismissText, {color: theme.colors.text}]}>{t('common.dismiss')}</Text>
        </TouchableOpacity>
        <TouchableOpacity
          onPress={handleReload}
          testID="fileChangedReloadButton"
          style={[styles.button, {backgroundColor: theme.colors.link}]}>
          <Text style={styles.buttonText}>{t('fileChanged.reload')}</Text>
        </TouchableOpacity>
      </View>
    </Animated.View>
  );
}

const styles = StyleSheet.create({
  container: {
    position: 'absolute',
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 14,
    paddingVertical: 10,
    borderRadius: 16,
    borderWidth: 1,
    shadowColor: '#000',
    shadowOffset: {width: 0, height: 2},
    shadowOpacity: 0.15,
    shadowRadius: 8,
    elevation: 4,
    gap: 10,
  },
  text: {fontSize: 14, fontWeight: '500'},
  actions: {flexDirection: 'row', alignItems: 'center', gap: 8},
  button: {paddingHorizontal: 14, paddingVertical: 6, borderRadius: 8},
  buttonText: {color: '#ffffff', fontSize: 13, fontWeight: '600'},
  dismissButton: {paddingHorizontal: 8, paddingVertical: 6},
  dismissText: {fontSize: 13},
});
