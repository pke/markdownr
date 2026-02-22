import React, {useState, useEffect, useCallback, useMemo, useRef} from 'react';
import {StatusBar, useColorScheme, Platform, View, ScrollView} from 'react-native';
import {SafeAreaProvider} from 'react-native-safe-area-context';
import {NavigationContainer, createNavigationContainerRef} from '@react-navigation/native';
import {createNativeStackNavigator} from '@react-navigation/native-stack';
import {createDrawerNavigator} from '@react-navigation/drawer';
import {File} from 'expo-file-system/next';
import * as Linking from 'expo-linking';

import {EXAMPLE_MARKDOWN} from './example';
import {Settings, SettingsKeys, addSettingsListener} from './settings';
import {addRecentFile, getRecentFiles, loadRecentFile} from './recentFiles';
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
  const [colorMode, setColorMode] = useState<string>(() =>
    Settings.getString(SettingsKeys.COLOR_MODE, 'system')
  );
  const isDarkMode = colorMode === 'system'
    ? systemColorScheme === 'dark'
    : colorMode === 'dark';
  const toggleDarkMode = useCallback(() => {
    const newMode = isDarkMode ? 'light' : 'dark';
    setColorMode(newMode);
    Settings.setString(SettingsKeys.COLOR_MODE, newMode);
  }, [isDarkMode]);

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
  const [markdownContent, setMarkdownContent] = useState<string>(initialRecent ? '' : EXAMPLE_MARKDOWN);
  const [fileName, setFileName] = useState<string | null>(initialRecent?.subtitle ?? null);
  const [frontMatterTheme, setFrontMatterTheme] = useState<ThemeConfig | null>(null);
  const [frontMatterThemeApplied, setFrontMatterThemeApplied] = useState(false);

  useEffect(() => {
    if (fileName === null) {
      setMarkdownContent(EXAMPLE_MARKDOWN);
    }
    // Reset applied state so the banner can re-appear for new files.
    // Don't reset frontMatterTheme here — ViewerScreen's effect handles
    // that based on the parsed front matter content.
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

  const openFile = useCallback((content: string, name: string | null) => {
    addRecentFile(content, name);
    setMarkdownContent(content);
    setFileName(name);
  }, []);

  const openedViaDeepLink = useRef(false);

  const handleInitialUrl = useCallback(
    async (url: string | null) => {
      if (!url) return;

      try {
        let filePath = url;

        if (Platform.OS === 'ios') {
          filePath = decodeURIComponent(url.replace('file://', ''));
        }

        const fileUri = Platform.OS === 'android' ? url : `file://${filePath}`;
        const content = await new File(fileUri).text();

        const name = url.split('/').pop() ?? 'Unknown';
        openFile(content, name);
        openedViaDeepLink.current = true;
      } catch (err) {
        console.error('Error reading file from URL:', err);
      }
    },
    [openFile],
  );

  useEffect(() => {
    Linking.getInitialURL().then((url) => {
      handleInitialUrl(url).then(() => {
        if (!openedViaDeepLink.current && initialRecent) {
          loadRecentFile(initialRecent).then((content) => {
            if (content) {
              setMarkdownContent(content);
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
  }, [handleInitialUrl, initialRecent]);

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
  }), [
    markdownContent, fileName, scrollToPercent, highlightText,
    searchMatches, currentMatchIndex, theme, backgroundColor,
    isDarkMode, toggleDarkMode, themeName, cycleTheme, showFrontMatterSetting,
    scrollToHeadingIndex, frontMatterTheme, frontMatterThemeApplied,
    applyTheme, openFile,
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
                        presentation: 'modal',
                        headerShown: false,
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
