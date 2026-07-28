import React, {useState, useEffect, useCallback, useMemo, useRef} from 'react';
import {AppState, StatusBar, useColorScheme, Platform, View, ScrollView, Settings as RNSettings} from 'react-native';
import {SafeAreaProvider} from 'react-native-safe-area-context';
import {NavigationContainer, createNavigationContainerRef} from '@react-navigation/native';
import {createNativeStackNavigator} from '@react-navigation/native-stack';
import {createDrawerNavigator} from '@react-navigation/drawer';
import {File, Paths} from 'expo-file-system/next';
import * as Linking from 'expo-linking';

import './i18n'; // initialize i18next before any screen renders
import {openDeepLink} from './deepLink';
import {
  checkForExternalChange, shouldShowBanner, statFileSource, readFileSource,
  reloadFromSource, type FileSource,
} from './fileChangeDetection';
import {getWelcomeMarkdown} from './example';
import {Settings, SettingsKeys, Storage, StorageKeys, addSettingsListener} from './settings';
import {addRecentFile, getRecentFiles, loadRecentFile, clearAllRecentFiles, getCachedFileMtime} from './recentFiles';
import {MarkdownContext, type SearchMatch, type TocHeading} from './MarkdownContext';
import {customThemes, themeNames, type ThemeName, type ThemeConfig} from './themes';
import {ViewerScreen} from './ViewerScreen';
import {SearchScreen} from './SearchScreen';
import {RecentFilesScreen} from './RecentFilesScreen';
import {TocDrawerContent} from './TocDrawer';
import {ErrorBoundary} from './ErrorBoundary';

const Stack = createNativeStackNavigator();
const Drawer = createDrawerNavigator();
const navigationRef = createNavigationContainerRef();

/** Classify a viewer URI for external-change detection. */
function sourceForUri(uri: string | null): FileSource | null {
  if (uri?.startsWith('file://')) return {uri, kind: 'file'};
  if (uri?.startsWith('content://')) return {uri, kind: 'content'};
  return null;
}

/** Rehydrate the persisted source descriptor (falls back to deriving one from
 * the last file URI, e.g. for state written before Phase 2). */
function restorePersistedSource(): FileSource | null {
  const json = Storage.getString(StorageKeys.LAST_FILE_SOURCE);
  if (json) {
    try {
      const parsed = JSON.parse(json) as FileSource;
      if (parsed?.uri && parsed?.kind) return parsed;
    } catch {
      // fall through to URI-derived source
    }
  }
  return sourceForUri(Storage.getString(StorageKeys.LAST_FILE_URI) || null);
}

export default function App() {
  const systemColorScheme = useColorScheme();
  const [colorMode, setColorMode] = useState<string>(() =>
    Settings.getString(SettingsKeys.COLOR_MODE, 'system')
  );
  const isDarkMode = colorMode === 'system'
    ? systemColorScheme === 'dark'
    : colorMode === 'dark';
  const toggleDarkMode = useCallback(() => {
    const next = colorMode === 'system' ? 'dark' : colorMode === 'dark' ? 'light' : 'system';
    setColorMode(next);
    Settings.setString(SettingsKeys.COLOR_MODE, next);
  }, [colorMode]);

  const [themeName, setThemeName] = useState<ThemeName>(() => {
    const saved = Settings.getString(SettingsKeys.THEME, 'default');
    return themeNames.includes(saved as ThemeName) ? (saved as ThemeName) : 'default';
  });
  const cycleTheme = useCallback(() => {
    const currentIndex = themeNames.indexOf(themeName);
    const nextIndex = (currentIndex + 1) % themeNames.length;
    const newTheme = themeNames[nextIndex];
    setThemeName(newTheme);
    Settings.setString(SettingsKeys.THEME, newTheme);
  }, [themeName]);

  const initialRecent = useMemo(() => getRecentFiles()[0] ?? null, []);
  const [markdownContent, setMarkdownContent] = useState<string>(initialRecent ? '' : getWelcomeMarkdown());
  const [fileName, setFileName] = useState<string | null>(initialRecent?.subtitle ?? null);
  const [currentFileUri, setCurrentFileUri] = useState<string | null>(() =>
    Storage.getString(StorageKeys.LAST_FILE_URI) || null
  );
  const [frontMatterTheme, setFrontMatterTheme] = useState<ThemeConfig | null>(null);
  const [frontMatterThemeApplied, setFrontMatterThemeApplied] = useState(false);

  useEffect(() => {
    if (fileName === null) {
      setMarkdownContent(getWelcomeMarkdown());
    }
    setFrontMatterThemeApplied(false);
  }, [fileName]);

  const [scrollToPercent, setScrollToPercent] = useState<number | null>(null);
  const [highlightText, setHighlightText] = useState<string | null>(null);
  const [searchMatches, setSearchMatches] = useState<SearchMatch[]>([]);
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);

  const tocHeadingsRef = useRef<TocHeading[]>([]);
  const [scrollToHeadingIndex, setScrollToHeadingIndex] = useState<number | null>(null);
  const headingRefsMap = useRef<Map<number, View>>(new Map());
  const scrollViewRef = useRef<ScrollView>(null);

  useEffect(() => { headingRefsMap.current.clear(); }, [markdownContent]);

  const [showFrontMatterSetting, setShowFrontMatterSetting] = useState(() =>
    Settings.getBoolean(SettingsKeys.SHOW_FRONT_MATTER)
  );

  useEffect(() => {
    Settings.setBoolean(SettingsKeys.SHOW_FRONT_MATTER, showFrontMatterSetting);
  }, [showFrontMatterSetting]);

  const reloadSettings = useCallback(() => {
    const newShowFrontMatter = Settings.getBoolean(SettingsKeys.SHOW_FRONT_MATTER);
    const newColorMode = Settings.getString(SettingsKeys.COLOR_MODE, 'system');
    const newTheme = Settings.getString(SettingsKeys.THEME, 'default');
    setShowFrontMatterSetting(newShowFrontMatter);
    setColorMode(newColorMode);
    if (themeNames.includes(newTheme as ThemeName)) {
      setThemeName(newTheme as ThemeName);
    }
  }, []);

  useEffect(() => {
    return addSettingsListener(reloadSettings);
  }, [reloadSettings]);

  // --- External-change detection (see docs/superpowers/specs/2026-07-13-…) ---
  // Baseline = mtime of the displayed snapshot; a newer source mtime on app
  // focus offers a reload. Refs, not state: only the banner visibility renders.
  const currentSourceRef = useRef<FileSource | null>(null);
  const baselineMtimeRef = useRef<number | null>(null);
  const dismissedMtimeRef = useRef<number | null>(null);
  const pendingMtimeRef = useRef<number | null>(null);
  const [externalChangeDetected, setExternalChangeDetected] = useState(false);

  const openFile = useCallback((content: string, name: string | null, fileUri?: string | null, sourceOverride?: FileSource | null) => {
    const uri = fileUri ?? null;
    const source = sourceOverride ?? sourceForUri(uri);
    addRecentFile(content, name, source ?? undefined);
    setMarkdownContent(content);
    setFileName(name);
    setCurrentFileUri(uri);
    Storage.setString(StorageKeys.LAST_FILE_URI, uri ?? '');
    Storage.setString(StorageKeys.LAST_FILE_SOURCE, source ? JSON.stringify(source) : '');

    currentSourceRef.current = source;
    baselineMtimeRef.current = null;
    dismissedMtimeRef.current = null;
    pendingMtimeRef.current = null;
    setExternalChangeDetected(false);
    if (source) {
      // Fire-and-forget: mtime of the file we just read is the new baseline.
      statFileSource(source).then(m => { baselineMtimeRef.current = m; }).catch(() => {});
    }
  }, []);

  const openedViaDeepLink = useRef(false);

  const handleInitialUrl = useCallback(
    async (url: string | null) => {
      if (!url) return;

      try {
        const opened = await openDeepLink(url, (uri) => new File(uri).text(), openFile);
        if (opened) openedViaDeepLink.current = true;
      } catch (err) {
        console.error('Error reading file from URL:', err);
      }
    },
    [openFile],
  );

  const runExternalChangeCheck = useCallback(async () => {
    const check = await checkForExternalChange(currentSourceRef.current, baselineMtimeRef.current);
    if (shouldShowBanner(check, dismissedMtimeRef.current)) {
      pendingMtimeRef.current = check.sourceMtime;
      setExternalChangeDetected(true);
    }
  }, []);

  useEffect(() => {
    const sub = AppState.addEventListener('change', (state) => {
      if (state === 'active') runExternalChangeCheck();
    });
    return () => sub.remove();
  }, [runExternalChangeCheck]);

  const dismissExternalChange = useCallback(() => {
    dismissedMtimeRef.current = pendingMtimeRef.current;
    setExternalChangeDetected(false);
  }, []);

  const reloadCurrentFile = useCallback(async () => {
    const source = currentSourceRef.current;
    if (!source) { setExternalChangeDetected(false); return; }
    const ok = await reloadFromSource(source, fileName, openFile, readFileSource);
    if (!ok) dismissExternalChange(); // read failed: keep content, stop nagging
  }, [fileName, openFile, dismissExternalChange]);

  useEffect(() => {
    Linking.getInitialURL().then((url) => {
      handleInitialUrl(url).then(() => {
        if (!openedViaDeepLink.current && initialRecent) {
          loadRecentFile(initialRecent).then((content) => {
            if (content) {
              setMarkdownContent(content);
              // The displayed snapshot is the recents cache copy; its mtime is
              // the baseline. An edit made while the app was quit is offered
              // for reload right away.
              currentSourceRef.current = restorePersistedSource();
              baselineMtimeRef.current = getCachedFileMtime(initialRecent.id);
              runExternalChangeCheck();
            } else {
              // Cached file was purged — fall back to welcome screen
              setFileName(null);
            }
          });
        }
      });
    });

    const subscription = Linking.addEventListener('url', ({url}) => {
      handleInitialUrl(url);
    });

    return () => {
      subscription.remove();
    };
  }, [handleInitialUrl, initialRecent, runExternalChangeCheck]);

  // DEBUG-only hook so UI tests can seed recent files without the native picker.
  // A test drops an empty sentinel file into the app's documents dir; on launch
  // we reset recents to a known set and remove the sentinel. No-op in release.
  useEffect(() => {
    if (!__DEV__) return;
    try {
      // Two triggers: a sentinel file (Maestro places it via recents-setup.sh)
      // or the `-uitestSeedRecents YES` launch argument (XCUITest), which iOS
      // exposes through NSUserDefaults / RN Settings.
      const sentinel = new File(Paths.document, '__uitest_seed_recents__');
      const byLaunchArg = Platform.OS === 'ios' && RNSettings.get('uitestSeedRecents') === 'YES';
      if (!sentinel.exists && !byLaunchArg) return;
      if (sentinel.exists) sentinel.delete();
      clearAllRecentFiles();
      addRecentFile('# Alpha\n\nFirst test note.', 'alpha.md');
      addRecentFile('# Beta\n\nSecond test note.', 'beta.md');
      addRecentFile('# Gamma\n\nThird test note.', 'gamma.md');
    } catch {
      // best-effort test seeding
    }
  }, []);

  const applyTheme = useCallback((name: ThemeName) => {
    setThemeName(name);
    Settings.setString(SettingsKeys.THEME, name);
  }, []);

  const activeThemeConfig = (frontMatterThemeApplied && frontMatterTheme)
    ? frontMatterTheme
    : customThemes[themeName];
  const theme = isDarkMode ? activeThemeConfig.dark : activeThemeConfig.light;
  const backgroundColor = isDarkMode ? activeThemeConfig.background.dark : activeThemeConfig.background.light;

  const contextValue = useMemo(() => ({
    markdownContent,
    setMarkdownContent,
    fileName,
    setFileName,
    currentFileUri,
    setCurrentFileUri,
    scrollToPercent,
    setScrollToPercent,
    highlightText,
    setHighlightText,
    searchMatches,
    setSearchMatches,
    currentMatchIndex,
    setCurrentMatchIndex,
    theme,
    backgroundColor,
    isDarkMode,
    colorMode: colorMode as 'system' | 'dark' | 'light',
    toggleDarkMode,
    themeName,
    cycleTheme,
    showFrontMatterSetting,
    tocHeadingsRef,
    scrollToHeadingIndex,
    setScrollToHeadingIndex,
    headingRefsMap,
    scrollViewRef,
    frontMatterTheme,
    setFrontMatterTheme,
    frontMatterThemeApplied,
    setFrontMatterThemeApplied,
    applyTheme,
    openFile,
    externalChangeDetected,
    reloadCurrentFile,
    dismissExternalChange,
  }), [
    markdownContent, fileName, currentFileUri, scrollToPercent, highlightText,
    searchMatches, currentMatchIndex, theme, backgroundColor,
    isDarkMode, colorMode, toggleDarkMode, themeName, cycleTheme, showFrontMatterSetting,
    scrollToHeadingIndex, frontMatterTheme, frontMatterThemeApplied,
    applyTheme, openFile,
    externalChangeDetected, reloadCurrentFile, dismissExternalChange,
  ]);

  const drawerType = Platform.OS === 'android' ? 'back' : 'slide';

  return (
    <ErrorBoundary>
      <MarkdownContext.Provider value={contextValue}>
        <SafeAreaProvider>
          <StatusBar barStyle={isDarkMode ? 'light-content' : 'dark-content'} />
          <NavigationContainer ref={navigationRef}>
            <Drawer.Navigator
              drawerContent={(props) => <TocDrawerContent {...props} />}
              screenOptions={{
                headerShown: false,
                drawerType,
                drawerStyle: {
                  backgroundColor,
                  width: 280,
                },
                swipeEnabled: true,
                swipeEdgeWidth: 50,
              }}>
              <Drawer.Screen name="Main">
                {() => (
                  <Stack.Navigator
                    screenOptions={{
                      headerShown: false,
                    }}>
                    <Stack.Screen
                      name="Viewer"
                      component={ViewerScreen}
                    />
                    <Stack.Screen
                      name="Search"
                      component={SearchScreen}
                      options={{
                        presentation: 'fullScreenModal',
                        headerShown: true,
                      }}
                    />
                    <Stack.Screen
                      name="RecentFiles"
                      component={RecentFilesScreen}
                      options={{
                        presentation: 'modal',
                        headerShown: false,
                      }}
                    />
                  </Stack.Navigator>
                )}
              </Drawer.Screen>
            </Drawer.Navigator>
          </NavigationContainer>
        </SafeAreaProvider>
      </MarkdownContext.Provider>
    </ErrorBoundary>
  );
}
