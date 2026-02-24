import React, {useState, useEffect, useCallback} from 'react';
import {StyleSheet, Text, TouchableOpacity, View} from 'react-native';
import Animated, {FadeInUp, FadeOutDown} from 'react-native-reanimated';
import {getLocales} from 'expo-localization';

import {MarkdownContext} from './MarkdownContext';
import {customThemes} from './themes';
import {parseFrontMatter} from './frontmatter';
import {resolveThemeFromFrontMatter, fetchRemoteTheme} from './themeLoader';
import {getSeasonalThemeSuggestion, type SeasonalSuggestion} from './seasonalTheme';

type ThemeSuggestionProps = {
  variant: 'banner' | 'drawer';
  bottomInset?: number;
  onApply?: () => void;
};

type Suggestion = {
  label: string;
  type: 'frontmatter' | 'seasonal';
  seasonal?: SeasonalSuggestion;
};

export function ThemeSuggestion({variant, bottomInset, onApply}: ThemeSuggestionProps) {
  const {
    markdownContent,
    fileName,
    themeName,
    theme,
    backgroundColor,
    frontMatterTheme,
    setFrontMatterTheme,
    frontMatterThemeApplied,
    setFrontMatterThemeApplied,
    applyTheme,
  } = React.useContext(MarkdownContext);

  // --- Front matter theme detection ---
  const frontMatterThemeValue = React.useMemo(() => {
    const {frontMatter} = parseFrontMatter(markdownContent);
    return frontMatter?.theme as string | undefined;
  }, [markdownContent]);

  const [fmLabel, setFmLabel] = useState<string | null>(null);
  const [fmDismissed, setFmDismissed] = useState(false);

  useEffect(() => {
    setFmDismissed(false);

    if (!frontMatterThemeValue) {
      setFrontMatterTheme(null);
      setFmLabel(null);
      return;
    }

    const resolved = resolveThemeFromFrontMatter(frontMatterThemeValue);
    if (!resolved) {
      setFrontMatterTheme(null);
      setFmLabel(null);
      return;
    }

    if (resolved.type === 'builtin') {
      setFrontMatterTheme(resolved.config);
      const emoji = customThemes[resolved.name].icon;
      const name = resolved.name.charAt(0).toUpperCase() + resolved.name.slice(1);
      setFmLabel(`${emoji} ${name}`);
    } else {
      setFmLabel(null);
      fetchRemoteTheme(resolved.url).then(config => {
        if (config) {
          setFrontMatterTheme(config);
          try {
            const pathname = new URL(resolved.url).pathname;
            const segment = pathname.split('/').filter(Boolean).pop() ?? '';
            const name = segment.replace(/\.json$/i, '').replace(/[-_]/g, ' ').trim();
            setFmLabel(name ? name.charAt(0).toUpperCase() + name.slice(1) : 'Custom theme');
          } catch {
            setFmLabel('Custom theme');
          }
        }
      });
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- setFrontMatterTheme is a stable context setter
  }, [frontMatterThemeValue, fileName]);

  // --- Seasonal suggestion (drawer only, suppressed when front matter declares a theme) ---
  const seasonalSuggestion = React.useMemo<SeasonalSuggestion | null>(() => {
    if (variant === 'banner') return null;
    if (frontMatterThemeValue) return null;
    const regionCode = getLocales()[0]?.regionCode ?? null;
    const suggestion = getSeasonalThemeSuggestion(regionCode);
    if (!suggestion || suggestion.themeName === themeName) return null;
    return suggestion;
  }, [themeName, variant, frontMatterThemeValue]);

  // --- Priority: front matter > seasonal ---
  // Banner: respect dismissal (hide after dismiss)
  // Drawer: ignore dismissal (always show front matter suggestion)
  const fmMatchesCurrent = frontMatterThemeValue === themeName;
  const showFm = frontMatterTheme !== null && !frontMatterThemeApplied && !fmMatchesCurrent
    && fmLabel !== null && (variant === 'drawer' || !fmDismissed);

  const suggestion = React.useMemo<Suggestion | null>(() => showFm
    ? {label: fmLabel!, type: 'frontmatter'}
    : seasonalSuggestion
      ? {label: seasonalSuggestion.label, type: 'seasonal', seasonal: seasonalSuggestion}
      : null,
  [showFm, fmLabel, seasonalSuggestion]);

  // --- Handlers ---
  const handleApply = useCallback(() => {
    if (!suggestion) return;
    if (suggestion.type === 'frontmatter') {
      setFrontMatterThemeApplied(true);
    } else if (suggestion.seasonal) {
      applyTheme(suggestion.seasonal.themeName);
    }
    onApply?.();
  // eslint-disable-next-line react-hooks/exhaustive-deps -- setFrontMatterThemeApplied is a stable context setter
  }, [suggestion, applyTheme, onApply]);

  const handleDismiss = useCallback(() => {
    if (!suggestion) return;
    if (suggestion.type === 'frontmatter') {
      setFmDismissed(true);
    }
  }, [suggestion]);

  if (!suggestion) return null;

  // --- Render ---
  if (variant === 'banner') {
    return (
      <Animated.View
        entering={FadeInUp.duration(300)}
        exiting={FadeOutDown.duration(200)}
        style={[
          bannerStyles.container,
          {
            bottom: bottomInset,
            right: 20,
            backgroundColor,
            borderColor: theme.colors.border,
          },
        ]}>
        <Text style={[bannerStyles.text, {color: theme.colors.text}]}>
          Suggested theme: {suggestion.label}
        </Text>
        <View style={bannerStyles.actions}>
          <TouchableOpacity onPress={handleDismiss} style={[bannerStyles.dismissButton, {borderColor: theme.colors.border}]}>
            <Text style={[bannerStyles.dismissText, {color: theme.colors.text}]}>Dismiss</Text>
          </TouchableOpacity>
          <TouchableOpacity onPress={handleApply} style={[bannerStyles.button, {backgroundColor: theme.colors.link}]}>
            <Text style={bannerStyles.buttonText}>Apply</Text>
          </TouchableOpacity>
        </View>
      </Animated.View>
    );
  }

  // Drawer variant
  return (
    <View style={[drawerStyles.container, {backgroundColor, borderColor: theme.colors.border}]}>
      <Text style={[drawerStyles.text, {color: theme.colors.text}]}>
        Suggested theme: {suggestion.label}
      </Text>
      <TouchableOpacity
        onPress={handleApply}
        style={[drawerStyles.button, {backgroundColor: theme.colors.link}]}
        activeOpacity={0.7}>
        <Text style={drawerStyles.buttonText}>Apply</Text>
      </TouchableOpacity>
    </View>
  );
}

const bannerStyles = StyleSheet.create({
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
  text: {
    fontSize: 14,
    fontWeight: '500',
  },
  actions: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  button: {
    paddingHorizontal: 14,
    paddingVertical: 6,
    borderRadius: 8,
  },
  buttonText: {
    color: '#ffffff',
    fontSize: 13,
    fontWeight: '600',
  },
  dismissButton: {
    paddingHorizontal: 8,
    paddingVertical: 6,
  },
  dismissText: {
    fontSize: 13,
  },
});

const drawerStyles = StyleSheet.create({
  container: {
    flexDirection: 'row',
    alignItems: 'center',
    marginHorizontal: 12,
    marginTop: 12,
    paddingHorizontal: 14,
    paddingVertical: 10,
    borderRadius: 16,
    borderWidth: 1,
    gap: 10,
  },
  text: {
    flex: 1,
    fontSize: 14,
    fontWeight: '500',
  },
  button: {
    paddingHorizontal: 14,
    paddingVertical: 6,
    borderRadius: 8,
  },
  buttonText: {
    color: '#ffffff',
    fontSize: 13,
    fontWeight: '600',
  },
});
