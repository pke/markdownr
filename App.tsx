import React, {useState, useEffect, useCallback, useMemo, useRef} from 'react';
import {AppState, StatusBar, useColorScheme, useWindowDimensions, Platform, View, ScrollView, Settings as RNSettings} from 'react-native';
import {SafeAreaProvider} from 'react-native-safe-area-context';
import {NavigationContainer, createNavigationContainerRef} from '@react-navigation/native';
import {createNativeStackNavigator} from '@react-navigation/native-stack';
import {createDrawerNavigator} from '@react-navigation/drawer';
import {File, Paths} from 'expo-file-system/next';
import * as Linking from 'expo-linking';
import {observeFolder} from 'folder-picker';

import './i18n'; // initialize i18next before any screen renders
import {openDeepLink, parseDeepLinkFileUri} from './deepLink';
import {
  createChangeTracker, readFileSource, reloadFromSource, sourceForUri, type FileSource,
} from './fileChangeDetection';
import {getWelcomeMarkdown} from './example';
import {Settings, SettingsKeys, Storage, StorageKeys, addSettingsListener} from './settings';
import {
  addRecentFile, getRecentFiles, clearAllRecentFiles, getCachedFileMtime, restoreRecentDocument,
} from './recentFiles';
import {folderHandle, folderIndex, readFolder, updateFolderLocation, type FolderLocation, type ViewerDocument} from './folderNavigation';
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

export default function App() {
  const systemColorScheme = useColorScheme();
  const {width: windowWidth} = useWindowDimensions();
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
  const [fileName, setFileName] = useState<string | null>(null);
  const [currentFileUri, setCurrentFileUri] = useState<string | null>(null);
  const [folderLocation, setFolderLocation] = useState<FolderLocation>();
  const folderLocationRef = useRef<FolderLocation | undefined>(undefined);
  const documentRevision = useRef(0);
  const [frontMatterTheme, setFrontMatterTheme] = useState<ThemeConfig | null>(null);
  const [frontMatterThemeApplied, setFrontMatterThemeApplied] = useState(false);

  useEffect(() => {
    setFrontMatterThemeApplied(false);
  }, [fileName, currentFileUri]);

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
  // focus offers a reload. A ref, not state: only the banner visibility renders.
  const changeTrackerRef = useRef(createChangeTracker());
  const [externalChangeDetected, setExternalChangeDetected] = useState(false);

  const displayDocument = useCallback(([content, name, uri, source, folder]: ViewerDocument, baseline?: number | null) => {
    documentRevision.current++;
    folderLocationRef.current = folder;
    setFolderLocation(folder);
    setMarkdownContent(content);
    setFileName(name);
    setCurrentFileUri(uri);
    changeTrackerRef.current.track(source, baseline);
    setExternalChangeDetected(false);
  }, []);

  const openFile = useCallback((content: string, name: string | null, uri: string | null = null,
    source: FileSource | null = sourceForUri(uri), folder?: FolderLocation,
  ) => {
    addRecentFile(content, name, source ?? undefined, folder);
    Storage.setString(StorageKeys.LAST_FILE_URI, uri ?? '');
    Storage.setString(StorageKeys.LAST_FILE_SOURCE, source ? JSON.stringify(source) : '');
    displayDocument([content, name, uri, source, folder]);
  }, [displayDocument]);

  const showLocalDocument = useCallback((content: string, name: string | null) => {
    displayDocument([content, name, null, null]);
  }, [displayDocument]);

  const runExternalChangeCheck = useCallback(async () => {
    const revision = documentRevision.current;
    if (await changeTrackerRef.current.check() && documentRevision.current === revision) {
      setExternalChangeDetected(true);
    }
  }, []);

  const activeFolder = folderLocation && folderHandle(folderLocation.result);
  const activeBookmark = folderLocation?.result.bookmarkKey;
  useEffect(() => {
    let disposed = false;
    let refreshing = false;
    let refreshAgain = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const refresh = async () => {
      if (disposed || AppState.currentState !== 'active') return;
      if (refreshing) { refreshAgain = true; return; }
      refreshing = true;
      try {
        if (activeFolder) {
          const result = await readFolder(activeFolder);
          if (disposed || AppState.currentState !== 'active') return;
          const next = result && updateFolderLocation(folderLocationRef.current, result);
          if (next) {
            folderLocationRef.current = next;
            setFolderLocation(next);
            if (!next.relativePath) {
              setMarkdownContent(folderIndex(next.result)[0]);
              setFileName(next.result.folderName);
            }
          }
        }
        await runExternalChangeCheck();
      } finally {
        refreshing = false;
        if (refreshAgain) {
          refreshAgain = false;
          void refresh();
        }
      }
    };

    const stopWatching = activeBookmark ? observeFolder(activeBookmark, () => {
      clearTimeout(timer);
      timer = setTimeout(refresh, 180);
    }) : undefined;
    // SAF providers are not required to notify observers of tree changes.
    const poll = activeFolder && Platform.OS === 'android' ? setInterval(refresh, 3000) : undefined;
    const sub = AppState.addEventListener('change', (state) => {
      if (state === 'active') void refresh();
    });
    if (activeFolder) void refresh();
    return () => {
      disposed = true;
      stopWatching?.();
      clearInterval(poll);
      clearTimeout(timer);
      sub.remove();
    };
  }, [activeFolder, activeBookmark, runExternalChangeCheck]);

  const dismissExternalChange = useCallback(() => {
    changeTrackerRef.current.dismiss();
    setExternalChangeDetected(false);
  }, []);

  const reloadCurrentFile = useCallback(async () => {
    const source = changeTrackerRef.current.source;
    if (!source) return false;
    const revision = documentRevision.current;
    const ok = await reloadFromSource(source, fileName, (content, name, uri, reloadedSource) => {
      if (documentRevision.current === revision) {
        openFile(content, name, uri, reloadedSource, folderLocationRef.current);
      }
    }, readFileSource);
    if (!ok && documentRevision.current === revision) dismissExternalChange();
    return ok && documentRevision.current === revision + 1 && changeTrackerRef.current.source === source;
  }, [fileName, openFile, dismissExternalChange]);

  useEffect(() => {
    let disposed = false;
    let linkRequest = 0;
    let pendingLink = Promise.resolve(false);
    const launchRevision = documentRevision.current;

    const receiveLink = (url: string | null) => {
      if (disposed || !parseDeepLinkFileUri(url)) return;
      const request = ++linkRequest;
      pendingLink = openDeepLink(url, (uri) => new File(uri).text(), (content, name, uri) => {
        if (!disposed && request === linkRequest) openFile(content, name, uri);
      }).catch((error: unknown) => {
        console.error('Error reading file from URL:', error);
        return false;
      });
    };

    const subscription = Linking.addEventListener('url', ({url}) => receiveLink(url));
    const restoreAtLaunch = async () => {
      const url = await Linking.getInitialURL().catch(() => null);
      if (!linkRequest) receiveLink(url);
      await pendingLink;
      if (disposed || documentRevision.current !== launchRevision || !initialRecent) return;

      const document = await restoreRecentDocument(initialRecent, Storage.getString(StorageKeys.LAST_FOLDER_URI));
      // A Linking event can arrive while the folder provider is being read.
      let waitFor: typeof pendingLink;
      do {
        waitFor = pendingLink;
        await waitFor;
      } while (waitFor !== pendingLink);
      if (disposed || documentRevision.current !== launchRevision) return;

      if (document?.[4]) {
        openFile(...document);
      } else if (document) {
        displayDocument(document, getCachedFileMtime(initialRecent.id));
        void runExternalChangeCheck();
      } else {
        showLocalDocument(getWelcomeMarkdown(), null);
      }
    };
    void restoreAtLaunch();

    return () => {
      disposed = true;
      subscription.remove();
    };
  }, [displayDocument, initialRecent, openFile, runExternalChangeCheck, showLocalDocument]);

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
    fileName,
    currentFileUri,
    folderLocation,
    showLocalDocument,
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
    markdownContent, fileName, currentFileUri, folderLocation, showLocalDocument, scrollToPercent, highlightText,
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
                swipeEdgeWidth: Platform.OS === 'android' ? windowWidth * 0.2 : 50,
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
