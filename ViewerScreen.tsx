import React, {useState, useEffect, useCallback, useRef} from 'react';
import {
  StyleSheet,
  View,
  ScrollView,
  Text,
  TouchableOpacity,
  Pressable,
  Alert,
  Platform,
  type NativeSyntheticEvent,
  type NativeScrollEvent,
} from 'react-native';
import {SymbolView, type SFSymbol} from 'expo-symbols';
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withTiming,
  withDelay,
  FadeIn,
  FadeOut,
  FadeInUp,
  FadeOutUp,
  LinearTransition,
} from 'react-native-reanimated';
import {useSafeAreaInsets} from 'react-native-safe-area-context';
import {useNavigation, DrawerActions} from '@react-navigation/native';
import {
  Markdown,
  Heading,
  Image as MarkdownImage,
  getTextContent,
  type MarkdownTheme,
  type MarkdownNode,
  type EnhancedRendererProps,
// @ts-ignore - importing from source to use patched astTransform
} from 'react-native-nitro-markdown/src';
import * as Linking from 'expo-linking';
import {Share} from 'react-native';
import {GlassView, isLiquidGlassAvailable} from 'expo-glass-effect';
import {GestureHandlerRootView} from 'react-native-gesture-handler';

import {MarkdownContext, type TocHeading} from './MarkdownContext';
import {customThemes, type ThemeConfig} from './themes';
import {parseFrontMatter, type FrontMatter, getExtraMetadata, hasExtraMetadata} from './frontmatter';
import {composeTransforms, createTypographicTransform, emoticonTransform, subSuperscriptTransform, abbreviationTransform, footnoteTransform, insMarkTransform, definitionListTransform, quoteCycleTransform, preprocessMarkdownHtml, tableBrTransform} from './astTransforms';
import {getLocales} from 'expo-localization';
import {ZoomableView} from './ZoomableView';
import {useFileOpener, useFolderOpener, resolveRelativeMarkdownLink} from './useFileOpener';
import {File} from 'expo-file-system/next';
import {ParticleOverlayProvider, ParticleOverlayBackground, ParticleOverlayForeground} from './ParticleOverlay';
import {SantaHat} from './SantaHat';
import {OCEAN_THEMED_SAMPLE} from './example';
import {getAboutMarkdown} from './about';
import {ThemeSuggestion} from './ThemeSuggestion';

const isIOS = Platform.OS === 'ios';

function MenuIcon({name, fallback, color}: {name: SFSymbol; fallback: string; color?: string}) {
  if (isIOS) {
    return <SymbolView name={name} size={22} tintColor={color ?? 'white'} />;
  }
  return <Text style={[styles.menuItemIcon, color ? {color} : undefined]}>{fallback}</Text>;
}

type FrontMatterBlockProps = {
  frontMatter: FrontMatter;
  onHide: () => void;
  isDarkMode: boolean;
  theme: MarkdownTheme;
};

function FrontMatterBlock({frontMatter, onHide, isDarkMode, theme}: FrontMatterBlockProps) {
  const [isExtraExpanded, setIsExtraExpanded] = useState(false);
  const extraMetadata = getExtraMetadata(frontMatter);
  const hasExtra = hasExtraMetadata(frontMatter);

  const containerBg = isDarkMode ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.04)';
  const borderColor = isDarkMode ? 'rgba(255,255,255,0.15)' : 'rgba(0,0,0,0.1)';
  const tagBg = isDarkMode ? 'rgba(255,255,255,0.15)' : 'rgba(0,0,0,0.08)';
  const metaColor = isDarkMode ? 'rgba(255,255,255,0.6)' : 'rgba(0,0,0,0.5)';

  return (
    <Animated.View
      entering={FadeInUp.duration(300)}
      exiting={FadeOutUp.duration(200)}
      layout={LinearTransition.duration(200)}
      style={[frontMatterStyles.container, {backgroundColor: containerBg, borderColor}]}>
      {frontMatter.tags && frontMatter.tags.length > 0 && (
        <View style={frontMatterStyles.tagsRow}>
          {frontMatter.tags.map((tag) => (
            <View key={tag} style={[frontMatterStyles.tag, {backgroundColor: tagBg}]}>
              <Text style={[frontMatterStyles.tagText, {color: theme.colors.text}]}>{tag}</Text>
            </View>
          ))}
        </View>
      )}

      {frontMatter.title && (
        <Text style={[frontMatterStyles.title, {color: theme.colors.heading}]}>
          {frontMatter.title}
        </Text>
      )}

      {(frontMatter.author || frontMatter.date) && (
        <View style={frontMatterStyles.metaRow}>
          {frontMatter.author && (
            <Text style={[frontMatterStyles.metaText, {color: metaColor}]}>
              {frontMatter.author}
            </Text>
          )}
          {frontMatter.author && frontMatter.date && (
            <Text style={[frontMatterStyles.metaText, {color: metaColor}]}> • </Text>
          )}
          {frontMatter.date && (
            <Text style={[frontMatterStyles.metaText, {color: metaColor}]}>
              {frontMatter.date}
            </Text>
          )}
        </View>
      )}

      {hasExtra && (
        <View style={frontMatterStyles.extraSection}>
          <TouchableOpacity
            onPress={() => setIsExtraExpanded(!isExtraExpanded)}
            style={frontMatterStyles.extraToggle}>
            <Text style={[frontMatterStyles.extraToggleText, {color: theme.colors.link}]}>
              {isExtraExpanded ? '▼' : '▶'} More info
            </Text>
          </TouchableOpacity>
          {isExtraExpanded && (
            <View style={frontMatterStyles.extraContent}>
              {Object.entries(extraMetadata).map(([key, value]) => (
                <View key={key} style={frontMatterStyles.extraItem}>
                  <Text style={[frontMatterStyles.extraKey, {color: metaColor}]}>{key}:</Text>
                  <Text style={[frontMatterStyles.extraValue, {color: theme.colors.text}]}>
                    {typeof value === 'object' ? JSON.stringify(value) : String(value)}
                  </Text>
                </View>
              ))}
            </View>
          )}
        </View>
      )}

      <TouchableOpacity onPress={onHide} style={frontMatterStyles.hideLink}>
        <Text style={[frontMatterStyles.hideLinkText, {color: metaColor}]}>hide</Text>
      </TouchableOpacity>
    </Animated.View>
  );
}

const frontMatterStyles = StyleSheet.create({
  container: {
    marginBottom: 16,
    padding: 12,
    borderRadius: 8,
    borderWidth: 1,
  },
  tagsRow: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    marginBottom: 8,
    gap: 6,
  },
  tag: {
    paddingHorizontal: 8,
    paddingVertical: 3,
    borderRadius: 12,
  },
  tagText: {
    fontSize: 12,
    fontWeight: '500',
  },
  title: {
    fontSize: 22,
    fontWeight: '700',
    marginBottom: 4,
  },
  metaRow: {
    flexDirection: 'row',
    alignItems: 'center',
    marginTop: 4,
  },
  metaText: {
    fontSize: 14,
  },
  extraSection: {
    marginTop: 12,
  },
  extraToggle: {
    paddingVertical: 4,
  },
  extraToggleText: {
    fontSize: 13,
    fontWeight: '500',
  },
  extraContent: {
    marginTop: 8,
    paddingLeft: 8,
  },
  extraItem: {
    flexDirection: 'row',
    marginBottom: 4,
  },
  extraKey: {
    fontSize: 13,
    fontWeight: '600',
    marginRight: 6,
  },
  extraValue: {
    fontSize: 13,
    flex: 1,
  },
  hideLink: {
    position: 'absolute',
    top: 10,
    right: 10,
  },
  hideLinkText: {
    fontSize: 12,
    fontStyle: 'italic',
  },
});

function HighlightedText({content, highlight, style}: {content: string; highlight: string | null; style?: any}) {
  if (!highlight || !content.toLowerCase().includes(highlight.toLowerCase())) {
    return <Text selectable style={style}>{content}</Text>;
  }

  const parts: React.ReactNode[] = [];
  let lastIndex = 0;
  const lowerContent = content.toLowerCase();
  const lowerHighlight = highlight.toLowerCase();
  let index = lowerContent.indexOf(lowerHighlight);

  while (index !== -1) {
    if (index > lastIndex) {
      parts.push(<Text key={`text-${lastIndex}`} style={style}>{content.slice(lastIndex, index)}</Text>);
    }
    parts.push(
      <Text key={`highlight-${index}`} style={[style, styles.highlight]}>
        {content.slice(index, index + highlight.length)}
      </Text>
    );
    lastIndex = index + highlight.length;
    index = lowerContent.indexOf(lowerHighlight, lastIndex);
  }

  if (lastIndex < content.length) {
    parts.push(<Text key={`text-${lastIndex}`} style={style}>{content.slice(lastIndex)}</Text>);
  }

  return <Text selectable style={style}>{parts}</Text>;
}

type SlideUpMenuItemProps = {
  isMenuOpen: boolean;
  delay: number;
  children: React.ReactNode;
};

const MAX_MENU_DELAY = 120;
const MENU_ANIM_DURATION = 100;

function SlideUpMenuItem({isMenuOpen, delay, children}: SlideUpMenuItemProps) {
  const opacity = useSharedValue(0);
  const translateY = useSharedValue(20);
  const scaledDelay = (delay / 240) * MAX_MENU_DELAY;
  const [shouldRender, setShouldRender] = React.useState(isMenuOpen);

  React.useEffect(() => {
    if (isMenuOpen) {
      setShouldRender(true);
      opacity.value = withDelay(scaledDelay, withTiming(1, {duration: MENU_ANIM_DURATION}));
      translateY.value = withDelay(scaledDelay, withTiming(0, {duration: MENU_ANIM_DURATION}));
    } else {
      const reverseDelay = MAX_MENU_DELAY - scaledDelay;
      opacity.value = withDelay(reverseDelay, withTiming(0, {duration: MENU_ANIM_DURATION}));
      translateY.value = withDelay(reverseDelay, withTiming(20, {duration: MENU_ANIM_DURATION}));
      const timeout = setTimeout(() => setShouldRender(false), MAX_MENU_DELAY + MENU_ANIM_DURATION);
      return () => clearTimeout(timeout);
    }
  }, [isMenuOpen, scaledDelay, opacity, translateY]);

  const animatedStyle = useAnimatedStyle(() => {
    'worklet';
    return {
      opacity: opacity.value,
      transform: [{translateY: translateY.value}],
    };
  });

  if (!shouldRender) return null;

  return (
    <Animated.View style={[styles.menuItemContainer, animatedStyle]}>
      {children}
    </Animated.View>
  );
}

type FloatingMenuProps = {
  isMenuVisible: boolean;
  isMenuOpen: boolean;
  setIsMenuOpen: (open: boolean) => void;
  setShowFrontMatter: (show: boolean) => void;
  showSource: boolean;
  toggleSource: () => void;
};

function FloatingMenu({isMenuVisible, isMenuOpen, setIsMenuOpen, setShowFrontMatter, showSource, toggleSource}: FloatingMenuProps) {
  const insets = useSafeAreaInsets();
  const navigation = useNavigation();
  const {showFrontMatterSetting, theme, isDarkMode, colorMode, toggleDarkMode, themeName, cycleTheme} = React.useContext(MarkdownContext);

  const menuOpacity = useSharedValue(1);
  const menuTranslateY = useSharedValue(0);

  useEffect(() => {
    if (isMenuVisible) {
      menuOpacity.value = withTiming(1, {duration: 200});
      menuTranslateY.value = withTiming(0, {duration: 200});
    } else {
      menuOpacity.value = withTiming(0, {duration: 200});
      menuTranslateY.value = withTiming(100, {duration: 200});
      setIsMenuOpen(false);
    }
  }, [isMenuVisible, menuOpacity, menuTranslateY, setIsMenuOpen]);

  const menuAnimatedStyle = useAnimatedStyle(() => {
    'worklet';
    return {
      opacity: menuOpacity.value,
      transform: [{translateY: menuTranslateY.value}],
    };
  });

  const toggleMenu = () => setIsMenuOpen(!isMenuOpen);
  const closeMenu = useCallback(() => setIsMenuOpen(false), [setIsMenuOpen]);

  const openFile = useFileOpener(useCallback(() => {
    closeMenu();
    setShowFrontMatter(showFrontMatterSetting);
  }, [closeMenu, setShowFrontMatter, showFrontMatterSetting]));

  const openFolder = useFolderOpener(useCallback(() => {
    closeMenu();
    setShowFrontMatter(showFrontMatterSetting);
  }, [closeMenu, setShowFrontMatter, showFrontMatterSetting]));

  const handleSearch = () => {
    closeMenu();
    navigation.navigate('Search' as never);
  };

  const surfaceColor = theme.colors.surface ?? (isDarkMode ? '#1a1a1a' : '#f5f5f5');
  const glassAvailable = isLiquidGlassAvailable();
  const bgColor = surfaceColor + 'e6';
  const mainButtonBgColor = glassAvailable ? 'transparent' : bgColor;

  return (
    <>
      {isMenuOpen && (
        <Pressable
          style={[StyleSheet.absoluteFill, {backgroundColor: 'rgba(0,0,0,0.3)'}]}
          onPress={closeMenu}
        />
      )}
      <Animated.View style={[styles.floatingMenuContainer, {bottom: insets.bottom}, menuAnimatedStyle]}>
        <View style={styles.menuItemsClip}>
          <SlideUpMenuItem isMenuOpen={isMenuOpen} delay={320}>
            <Text style={[styles.menuItemLabel, {color: theme.colors.text, backgroundColor: bgColor}]}>{themeName.charAt(0).toUpperCase() + themeName.slice(1)}</Text>
            <TouchableOpacity
              onPress={() => { cycleTheme(); }}
              style={[styles.menuItem, {backgroundColor: bgColor}]}
              activeOpacity={0.7}
              testID="cycleThemeButton">
              <Text style={styles.menuItemIcon}>{customThemes[themeName].icon}</Text>
            </TouchableOpacity>
          </SlideUpMenuItem>

          <SlideUpMenuItem isMenuOpen={isMenuOpen} delay={240}>
            <Text style={[styles.menuItemLabel, {color: theme.colors.text, backgroundColor: bgColor}]}>
              {colorMode === 'system' ? 'Auto' : colorMode === 'dark' ? 'Dark' : 'Light'}
            </Text>
            <TouchableOpacity
              onPress={() => { toggleDarkMode(); }}
              style={[styles.menuItem, {backgroundColor: bgColor}]}
              activeOpacity={0.7}
              testID="darkModeButton">
              <MenuIcon
                name={colorMode === 'system' ? 'circle.lefthalf.filled' : colorMode === 'dark' ? 'moon' : 'sun.max'}
                fallback={colorMode === 'system' ? '⚙️' : colorMode === 'dark' ? '🌙' : '☀️'}
                color={theme.colors.text}
              />
            </TouchableOpacity>
          </SlideUpMenuItem>

          <SlideUpMenuItem isMenuOpen={isMenuOpen} delay={160}>
            <Text style={[styles.menuItemLabel, {color: theme.colors.text, backgroundColor: bgColor}]}>{showSource ? 'Rendered' : 'Source'}</Text>
            <TouchableOpacity
              onPress={() => { toggleSource(); closeMenu(); }}
              style={[styles.menuItem, {backgroundColor: bgColor}]}
              activeOpacity={0.7}
              testID="sourceButton">
              <MenuIcon name={showSource ? 'doc.richtext' : 'curlybraces'} fallback={showSource ? '📄' : '{ }'} color={theme.colors.text} />
            </TouchableOpacity>
          </SlideUpMenuItem>

          <SlideUpMenuItem isMenuOpen={isMenuOpen} delay={80}>
            <Text style={[styles.menuItemLabel, {color: theme.colors.text, backgroundColor: bgColor}]}>Open</Text>
            <TouchableOpacity
              onPress={openFile}
              style={[styles.menuItem, {backgroundColor: bgColor}]}
              activeOpacity={0.7}
              testID="openFileButton">
              <MenuIcon name="folder" fallback="📁" color={theme.colors.text} />
            </TouchableOpacity>
          </SlideUpMenuItem>

          <SlideUpMenuItem isMenuOpen={isMenuOpen} delay={40}>
            <Text style={[styles.menuItemLabel, {color: theme.colors.text, backgroundColor: bgColor}]}>Open Folder</Text>
            <TouchableOpacity
              onPress={openFolder}
              style={[styles.menuItem, {backgroundColor: bgColor}]}
              activeOpacity={0.7}
              testID="openFolderButton">
              <MenuIcon name="folder.badge.plus" fallback="📂" color={theme.colors.text} />
            </TouchableOpacity>
          </SlideUpMenuItem>

          <SlideUpMenuItem isMenuOpen={isMenuOpen} delay={0}>
            <Text style={[styles.menuItemLabel, {color: theme.colors.text, backgroundColor: bgColor}]}>Search</Text>
            <TouchableOpacity
              onPress={handleSearch}
              style={[styles.menuItem, {backgroundColor: bgColor}]}
              activeOpacity={0.7}
              testID="searchButton">
              <MenuIcon name="magnifyingglass" fallback="🔍" color={theme.colors.text} />
            </TouchableOpacity>
          </SlideUpMenuItem>
        </View>

        <GlassView glassEffectStyle="regular" isInteractive style={[styles.mainMenuButton, {backgroundColor: mainButtonBgColor}]}>
          <TouchableOpacity onPress={toggleMenu} activeOpacity={0.9} style={styles.menuItemTouchable} testID="mainMenuButton" accessible={true}>
            {isIOS ? (
              <SymbolView name={isMenuOpen ? 'chevron.down' : 'ellipsis'} size={18} tintColor={theme.colors.text} />
            ) : (
              <Text style={[styles.mainMenuButtonText, {color: theme.colors.text}]}>{isMenuOpen ? '▼' : '…'}</Text>
            )}
          </TouchableOpacity>
        </GlassView>
      </Animated.View>
    </>
  );
}

// ---------------------------------------------------------------------------
// QuoteCycler – renders one blockquote at a time, crossfading every 10s
// ---------------------------------------------------------------------------

function QuoteCycler({quotes, Renderer}: {quotes: MarkdownNode[]; Renderer: React.ComponentType<any>}) {
  const [index, setIndex] = useState(0);
  const [height, setHeight] = useState<number | undefined>(undefined);

  const longestIdx = React.useMemo(() => {
    let maxLen = 0;
    let idx = 0;
    quotes.forEach((q, i) => {
      const len = getTextContent(q).length;
      if (len > maxLen) { maxLen = len; idx = i; }
    });
    return idx;
  }, [quotes]);

  useEffect(() => {
    if (quotes.length <= 1) return;
    const interval = setInterval(() => setIndex(i => (i + 1) % quotes.length), 10000);
    return () => clearInterval(interval);
  }, [quotes.length]);

  return (
    <View style={height != null ? {height} : undefined}>
      {height == null && (
        <View style={{position: 'absolute', opacity: 0, left: 0, right: 0}} pointerEvents="none"
          onLayout={e => setHeight(e.nativeEvent.layout.height)}>
          <Renderer node={quotes[longestIdx]} depth={0} inListItem={false} parentIsText={false} />
        </View>
      )}
      <Animated.View key={index} entering={FadeIn.duration(600)} exiting={FadeOut.duration(600)}>
        <Renderer node={quotes[index]} depth={0} inListItem={false} parentIsText={false} />
      </Animated.View>
    </View>
  );
}

export function ViewerScreen() {
  const insets = useSafeAreaInsets();
  const navigation = useNavigation();
  const {
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
    colorMode,
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
    frontMatterThemeApplied,
    currentFileUri,
    openFile,
  } = React.useContext(MarkdownContext);

  const openFilePicker = useFileOpener();
  const openFolderPicker = useFolderOpener();

  const handleLinkPress = useCallback(async (href: string): Promise<boolean> => {
    // Try to resolve relative .md links against the current file's URI
    const resolved = resolveRelativeMarkdownLink(href, currentFileUri);
    if (resolved) {
      try {
        const content = await new File(resolved).text();
        const name = resolved.split('/').pop() ?? null;
        openFile(content, name, resolved);
        return false;
      } catch {
        Alert.alert(
          'Open Folder',
          'Select all files in this folder to enable link navigation.',
          [
            {text: 'Cancel', style: 'cancel'},
            {text: 'Open Folder', onPress: openFolderPicker},
          ],
        );
        return false;
      }
    }
    return true; // let non-md links open normally
  }, [currentFileUri, openFile, openFolderPicker]);

  const openSearch = useCallback(() => {
    navigation.navigate('Search' as never);
  }, [navigation]);

  // Local state for menu visibility (only used in this screen)
  const [isMenuVisible, setIsMenuVisible] = useState(true);
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const [showSource, setShowSource] = useState(false);
  const toggleSource = useCallback(() => setShowSource(s => !s), []);
  const sourceProgress = useSharedValue(0);
  useEffect(() => {
    sourceProgress.value = withTiming(showSource ? 1 : 0, {duration: 500});
  }, [showSource, sourceProgress]);
  const renderedStyle = useAnimatedStyle(() => ({opacity: 1 - sourceProgress.value}));
  const sourceStyle = useAnimatedStyle(() => ({opacity: sourceProgress.value}));

  // Local state for front matter visibility, reset when file changes
  const [showFrontMatter, setShowFrontMatter] = useState(showFrontMatterSetting);
  useEffect(() => {
    setShowFrontMatter(showFrontMatterSetting);
  }, [fileName, showFrontMatterSetting]);

  const animatedBgColor = useSharedValue(backgroundColor);
  React.useEffect(() => {
    animatedBgColor.value = withTiming(backgroundColor, {duration: 300});
  }, [backgroundColor, animatedBgColor]);

  const animatedBgStyle = useAnimatedStyle(() => ({
    backgroundColor: animatedBgColor.value,
  }));

  const {frontMatter, markdown: rawMarkdown} = React.useMemo(() => parseFrontMatter(markdownContent), [markdownContent]);
  const markdown = React.useMemo(() => preprocessMarkdownHtml(rawMarkdown), [rawMarkdown]);

  useEffect(() => {
    scrollViewRef.current?.scrollTo({y: 0, animated: false});
  }, [markdownContent, scrollViewRef]);

  const handleHideFrontMatter = () => {
    setShowFrontMatter(false);
  };

  const headingIndexCounter = useRef(0);
  const footnoteAnchors = useRef(new Map<string, View>());

  const handleParseComplete = useCallback(({ast}: {raw: string; ast: MarkdownNode; text: string}) => {
    const headings: TocHeading[] = [];
    let idx = 0;
    function walk(node: MarkdownNode) {
      if (node.type === 'heading') {
        headings.push({index: idx, level: node.level ?? 1, text: getTextContent(node)});
        idx++;
      }
      node.children?.forEach(walk);
    }
    walk(ast);
    tocHeadingsRef.current = headings;
    headingIndexCounter.current = 0;
  }, [tocHeadingsRef]);

  const [contentHeight, setContentHeight] = useState(0);
  const lastScrollY = useRef(0);

  const handleScroll = useCallback((event: NativeSyntheticEvent<NativeScrollEvent>) => {
    const currentY = event.nativeEvent.contentOffset.y;
    const delta = currentY - lastScrollY.current;

    if (Math.abs(delta) > 10) {
      const isScrollingDown = delta > 0;
      if (isScrollingDown && currentY > 50) {
        setIsMenuVisible(false);
      } else if (!isScrollingDown) {
        setIsMenuVisible(true);
      }
    }
    lastScrollY.current = currentY;
  }, [setIsMenuVisible]);

  const hasSearchResults = highlightText !== null && searchMatches.length > 0;
  const [shouldRenderControls, setShouldRenderControls] = useState(false);
  const controlsOpacity = useSharedValue(0);
  const controlsTranslateY = useSharedValue(150);

  useEffect(() => {
    if (hasSearchResults) {
      setShouldRenderControls(true);
      controlsTranslateY.value = withTiming(0, {duration: 200});
      controlsOpacity.value = withTiming(1, {duration: 200});
    } else {
      controlsTranslateY.value = withTiming(150, {duration: 200});
      controlsOpacity.value = withDelay(100, withTiming(0, {duration: 150}));
      const timer = setTimeout(() => setShouldRenderControls(false), 300);
      return () => clearTimeout(timer);
    }
  }, [hasSearchResults, controlsOpacity, controlsTranslateY]);

  const controlsAnimatedStyle = useAnimatedStyle(() => {
    'worklet';
    return {
      opacity: controlsOpacity.value,
      transform: [{translateY: controlsTranslateY.value}],
    };
  });

  useEffect(() => {
    if (scrollToPercent !== null && contentHeight > 0) {
      const yOffset = contentHeight * scrollToPercent;
      scrollViewRef.current?.scrollTo({y: yOffset, animated: true});
      setScrollToPercent(null);
    }
  }, [scrollToPercent, contentHeight, setScrollToPercent]);

  useEffect(() => {
    if (scrollToHeadingIndex === null) return;
    const viewRef = headingRefsMap.current.get(scrollToHeadingIndex);
    if (viewRef) {
      viewRef.measureInWindow((_x: number, headingY: number) => {
        (scrollViewRef.current as any)?.measureInWindow((_sx: number, scrollViewY: number) => {
          const offset = headingY - scrollViewY + lastScrollY.current;
          scrollViewRef.current?.scrollTo({y: Math.max(0, offset - 16), animated: true});
        });
      });
    }
    setScrollToHeadingIndex(null);
  }, [scrollToHeadingIndex, headingRefsMap, scrollViewRef, setScrollToHeadingIndex]);

  const scrollToAnchor = useCallback((anchor: string) => {
    const viewRef = footnoteAnchors.current.get(anchor);
    if (!viewRef || !scrollViewRef.current) return;
    viewRef.measureLayout(
      scrollViewRef.current as any,
      (_x: number, y: number) => {
        scrollViewRef.current?.scrollTo({y: Math.max(0, y - insets.top - 32), animated: true});
      },
      () => {},
    );
  }, [scrollViewRef, insets.top]);

  const handleContentSizeChange = (_width: number, height: number) => {
    setContentHeight(height);
  };

  const goToPrevMatch = () => {
    if (searchMatches.length === 0) return;
    const newIndex = currentMatchIndex > 0 ? currentMatchIndex - 1 : searchMatches.length - 1;
    setCurrentMatchIndex(newIndex);
    setScrollToPercent(searchMatches[newIndex].percent);
  };

  const goToNextMatch = () => {
    if (searchMatches.length === 0) return;
    const newIndex = currentMatchIndex < searchMatches.length - 1 ? currentMatchIndex + 1 : 0;
    setCurrentMatchIndex(newIndex);
    setScrollToPercent(searchMatches[newIndex].percent);
  };

  const isTextOnlyNode = (node: MarkdownNode): boolean => {
    if (!node.children || node.children.length === 0) {
      return node.type === 'text';
    }
    return node.children.every(child =>
      child.type === 'text' ||
      child.type === 'bold' ||
      child.type === 'italic' ||
      child.type === 'strikethrough' ||
      (child.children && isTextOnlyNode(child))
    );
  };

  const handleImageLongPress = useCallback((url: string) => {
    Share.share({url});
  }, []);

  const astTransformFn = React.useMemo(() => {
    const locale = getLocales()[0]?.languageCode ?? 'en';
    return composeTransforms(quoteCycleTransform, definitionListTransform, footnoteTransform, insMarkTransform, createTypographicTransform(locale), emoticonTransform, abbreviationTransform, subSuperscriptTransform, tableBrTransform);
  }, []);

  const activeConfig: ThemeConfig = (frontMatterThemeApplied && frontMatterTheme)
    ? frontMatterTheme
    : customThemes[themeName];
  const overlayConfig = activeConfig.overlay;
  const rendererConfig = activeConfig.customRenderers;
  const overlayColor = overlayConfig?.color
    ? typeof overlayConfig.color === 'string'
      ? overlayConfig.color
      : isDarkMode ? overlayConfig.color.dark : overlayConfig.color.light
    : undefined;

  const customRenderers = React.useMemo(() => ({
    image: ({url, alt, title, Renderer: R}: EnhancedRendererProps) => {
      return (
        <Pressable onLongPress={() => handleImageLongPress(url ?? '')} style={{flex: 1, minWidth: 120}}>
          <MarkdownImage url={url ?? ''} alt={alt} title={title} Renderer={R} />
        </Pressable>
      );
    },
    heading: ({node, level, Renderer}: EnhancedRendererProps) => {
      const idx = headingIndexCounter.current++;
      const decoration = rendererConfig?.headingDecoration;
      const showDecoration = decoration && (level ?? 1) === 1;
      // If the heading contains a single link child, iOS can't propagate taps
      // through nested <Text> elements. Lift the link press to a Pressable wrapper.
      const singleLinkChild = node.children?.length === 1 && node.children[0].type === 'link'
        ? node.children[0]
        : null;
      const headingContent = (
        <Heading level={level ?? 1}>
          {node.children?.map((child: MarkdownNode, i: number) => (
            <Renderer key={`${child.type}-${i}`} node={child} depth={1} inListItem={false} parentIsText={true} />
          ))}
        </Heading>
      );
      return (
        <View
          ref={(ref: View | null) => {
            if (ref) headingRefsMap.current.set(idx, ref);
            else headingRefsMap.current.delete(idx);
          }}
          collapsable={false}>
          {showDecoration && decoration.type === 'santa-hat' && (
            <View
              style={{position: 'absolute', top: 10, left: -15, zIndex: 1, transform: [{rotate: '-20deg'}]}}
              pointerEvents="none">
              <SantaHat size={decoration.size} />
            </View>
          )}
          {singleLinkChild ? (
            <Pressable onPress={() => handleLinkPress(singleLinkChild.href ?? '')}>
              {headingContent}
            </Pressable>
          ) : headingContent}
        </View>
      );
    },
    horizontal_rule: rendererConfig?.horizontalRule ? () => {
      const emojis = rendererConfig.horizontalRule!.emojis;
      return (
        <View style={{flexDirection: 'row', alignItems: 'center', marginVertical: 24}}>
          {emojis.map((emoji, i) => (
            <React.Fragment key={i}>
              <View style={{flex: 1, height: 1, backgroundColor: theme.colors.border}} />
              <Text style={{marginHorizontal: 8, fontSize: 14}}>{emoji}</Text>
            </React.Fragment>
          ))}
          <View style={{flex: 1, height: 1, backgroundColor: theme.colors.border}} />
        </View>
      );
    } : undefined,
    quote_cycle: ({node, Renderer}: EnhancedRendererProps) => {
      const quotes = node.children?.filter((c: MarkdownNode) => c.type === 'blockquote') ?? [];
      if (quotes.length === 0) return null;
      return <QuoteCycler quotes={quotes} Renderer={Renderer} />;
    },
    text: highlightText ? ({node}: {node: {content?: string}}) => {
      const content = node.content || '';
      return <HighlightedText content={content} highlight={highlightText} style={{color: theme.colors.text}} />;
    } : undefined,
    link: ({node, href, children}: {node: MarkdownNode; href?: string; children?: React.ReactNode}) => {
      // Footnote reference in body: #footnote-N@O → scrolls to footnote-N, registers as footnote-ref-N-O
      const refMatch = href?.match(/^#footnote-(\d+)@(\d+)$/);
      if (refMatch) {
        const num = refMatch[1];
        const occ = refMatch[2];
        const content = getTextContent(node);
        return (
          <View
            ref={(ref: View | null) => {
              const key = `footnote-ref-${num}-${occ}`;
              if (ref) footnoteAnchors.current.set(key, ref);
              else footnoteAnchors.current.delete(key);
            }}
            collapsable={false}
            style={{flexDirection: 'row'}}>
            <Text
              onPress={() => scrollToAnchor(`footnote-${num}`)}
              style={{color: theme.colors.link}}>
              {content}
            </Text>
          </View>
        );
      }
      // Footnote back-link in footer: #footnote-ref-N-O → scrolls to footnote-ref-N-O, registers as footnote-N
      const backMatch = href?.match(/^#(footnote-ref-(\d+)-\d+)$/);
      if (backMatch) {
        const target = backMatch[1];
        const num = backMatch[2];
        const content = getTextContent(node);
        return (
          <View
            ref={(ref: View | null) => {
              const key = `footnote-${num}`;
              if (ref) footnoteAnchors.current.set(key, ref);
              else footnoteAnchors.current.delete(key);
            }}
            collapsable={false}
            style={{flexDirection: 'row'}}>
            <Text
              onPress={() => scrollToAnchor(target)}
              style={{color: theme.colors.link}}>
              {content}
            </Text>
          </View>
        );
      }

      // Abbreviation: #abbr-TERM|Full expansion → double underline, long-press shows tooltip
      const abbrMatch = href?.match(/^#abbr-([^|]+)\|(.+)$/);
      if (abbrMatch) {
        const term = abbrMatch[1];
        const full = abbrMatch[2];
        return (
          <Text
            onPress={() => Alert.alert(term, full)}
            style={{
              textDecorationLine: 'underline',
              textDecorationStyle: 'double',
              color: theme.colors.text,
            }}>
            {term}
          </Text>
        );
      }

      const action = href?.match(/^markdownr:(.+)$/)?.[1];
      if (action) {
        const handlePress = () => {
          switch (action) {
            case 'menu':
              setIsMenuVisible(true);
              setIsMenuOpen(true);
              break;
            case 'toggle-frontmatter':
              const newValue = !showFrontMatter;
              setShowFrontMatter(newValue);
              if (newValue) setScrollToPercent(0);
              break;
            case 'toc':
              navigation.dispatch(DrawerActions.openDrawer());
              break;
            case 'darkmode':
              toggleDarkMode();
              break;
            case 'theme':
              cycleTheme();
              break;
            case 'open':
              openFilePicker();
              break;
            case 'search':
              openSearch();
              break;
            case 'sample-ocean':
              setMarkdownContent(OCEAN_THEMED_SAMPLE);
              setFileName('ocean-demo.md');
              break;
            case 'home':
              setFileName(null);
              break;
            case 'about':
              setMarkdownContent(getAboutMarkdown());
              setFileName('about.md');
              break;
          }
        };
        const content = getTextContent(node);
        let suffix = '';
        if (action === 'darkmode') {
          suffix = colorMode === 'system' ? ' dark mode 🌙' : colorMode === 'dark' ? ' light mode ☀️' : ' auto mode ⚙️';
        } else if (action === 'theme') {
          const emoji = customThemes[themeName].icon;
          const displayName = themeName.charAt(0).toUpperCase() + themeName.slice(1);
          suffix = ` color themes: ${emoji} ${displayName}`;
        }
        return (
          <>
            <Text onPress={handlePress} style={{color: theme.colors.link}}>
              {content}
            </Text>
            {suffix && <Text style={{color: theme.colors.text}}>{suffix}</Text>}
          </>
        );
      }

      if (highlightText && isTextOnlyNode(node)) {
        const content = getTextContent(node);
        return (
          <Text
            onPress={() => { if (href) Linking.openURL(href); }}
            onLongPress={() => { if (href) Share.share({url: href}); }}
            style={{color: theme.colors.link}}>
            {content}
          </Text>
        );
      }

      // Regular link: open on press, share on long press
      if (href) {
        return (
          <Text
            onPress={() => Linking.openURL(href)}
            onLongPress={() => Share.share({url: href})}
            style={{color: theme.colors.link, textDecorationLine: 'underline'}}>
            {children}
          </Text>
        );
      }

      return undefined;
    },
    ins: ({children}: EnhancedRendererProps) => {
      return (
        <Text style={{textDecorationLine: 'underline'}}>
          {children}
        </Text>
      );
    },
    mark: ({children}: EnhancedRendererProps) => {
      return (
        <Text style={{backgroundColor: 'rgba(255, 255, 0, 0.3)'}}>
          {children}
        </Text>
      );
    },
    dl: ({children}: EnhancedRendererProps) => {
      return (
        <View style={{marginVertical: 8}}>
          {children}
        </View>
      );
    },
    dt: ({children}: EnhancedRendererProps) => {
      return (
        <Text style={{fontWeight: 'bold', color: theme.colors.text, marginTop: 8}}>
          {children}
        </Text>
      );
    },
    dd: ({children}: EnhancedRendererProps) => {
      return (
        <View style={{paddingLeft: 24, marginTop: 2}}>
          {children}
        </View>
      );
    },
  }), [highlightText, theme.colors.text, theme.colors.link, theme.colors.heading, theme.colors.border, setIsMenuVisible, setIsMenuOpen, showFrontMatter, setShowFrontMatter, setScrollToPercent, navigation, toggleDarkMode, cycleTheme, isDarkMode, colorMode, themeName, rendererConfig, openFilePicker, openSearch, headingRefsMap, scrollToAnchor]);

  const shouldShowFrontMatter = showFrontMatter && frontMatter;

  const content = (
    <Animated.View style={[styles.container, animatedBgStyle]}>
      <GestureHandlerRootView style={styles.container}>
        {overlayConfig && <ParticleOverlayBackground color={overlayColor} />}
        <ZoomableView>
          <ScrollView
            ref={scrollViewRef}
            style={styles.scrollView}
            contentContainerStyle={[styles.scrollContent, {paddingTop: insets.top + 16}]}
            onContentSizeChange={handleContentSizeChange}
            onScroll={handleScroll}
            scrollEventThrottle={16}>
            {shouldShowFrontMatter && (
              <FrontMatterBlock
                frontMatter={frontMatter}
                onHide={handleHideFrontMatter}
                isDarkMode={isDarkMode}
                theme={theme}
              />
            )}
            {showSource ? (
              <Animated.View style={sourceStyle}>
                <Text
                  style={{
                    fontFamily: theme.fontFamilies.mono,
                    fontSize: theme.fontSizes.s,
                    color: theme.colors.text,
                    lineHeight: theme.fontSizes.s * 1.6,
                    padding: 16,
                  }}
                  selectable>
                  {markdown}
                </Text>
              </Animated.View>
            ) : (
              <Animated.View style={renderedStyle}>
                <Markdown
                  theme={theme}
                  options={{gfm: true, math: true}}
                  astTransform={astTransformFn}
                  renderers={customRenderers}
                  onParseComplete={handleParseComplete}
                  onLinkPress={handleLinkPress}
                  style={styles.markdown}>
                  {markdown}
                </Markdown>
              </Animated.View>
            )}
          </ScrollView>
        </ZoomableView>

        {frontMatter && !showFrontMatter && (
          <TouchableOpacity
            style={[styles.infoButton, {top: insets.top + 8, backgroundColor: (theme.colors.surface ?? (isDarkMode ? '#1a1a1a' : '#f5f5f5')) + 'cc'}]}
            onPress={() => setShowFrontMatter(true)}
            activeOpacity={0.7}>
            <Text style={[styles.infoButtonText, {color: theme.colors.text}]}>ⓘ</Text>
          </TouchableOpacity>
        )}
        {shouldRenderControls && (
          <View style={[styles.searchControlsClip, {bottom: insets.bottom + 76}]}>
            <Animated.View style={[styles.searchControls, controlsAnimatedStyle]}>
              <TouchableOpacity onPress={() => { setHighlightText(null); setSearchMatches([]); }} style={[styles.searchControlButton, {backgroundColor: (theme.colors.surface ?? (isDarkMode ? '#1a1a1a' : '#f5f5f5')) + 'e6'}]}>
                <Text style={[styles.searchControlButtonText, {color: theme.colors.text}]}>✕</Text>
              </TouchableOpacity>
              <Text style={[styles.matchCount, {backgroundColor: (theme.colors.surface ?? (isDarkMode ? '#1a1a1a' : '#f5f5f5')) + 'e6', color: theme.colors.text}]}>
                {currentMatchIndex + 1}/{searchMatches.length}
              </Text>
              <TouchableOpacity onPress={goToPrevMatch} style={[styles.searchControlButton, {backgroundColor: (theme.colors.surface ?? (isDarkMode ? '#1a1a1a' : '#f5f5f5')) + 'e6'}]}>
                <Text style={[styles.searchControlButtonText, {color: theme.colors.text}]}>▲</Text>
              </TouchableOpacity>
              <TouchableOpacity onPress={goToNextMatch} style={[styles.searchControlButton, {backgroundColor: (theme.colors.surface ?? (isDarkMode ? '#1a1a1a' : '#f5f5f5')) + 'e6'}]}>
                <Text style={[styles.searchControlButtonText, {color: theme.colors.text}]}>▼</Text>
              </TouchableOpacity>
            </Animated.View>
          </View>
        )}
        <ThemeSuggestion variant="banner" bottomInset={insets.bottom + 56 + 12} />
        <FloatingMenu
          isMenuVisible={isMenuVisible}
          isMenuOpen={isMenuOpen}
          setIsMenuOpen={setIsMenuOpen}
          setShowFrontMatter={setShowFrontMatter}
          showSource={showSource}
          toggleSource={toggleSource}
        />
        {overlayConfig && <ParticleOverlayForeground color={overlayColor} />}
      </GestureHandlerRootView>
    </Animated.View>
  );

  return overlayConfig
    ? <ParticleOverlayProvider key={themeName} config={overlayConfig}>{content}</ParticleOverlayProvider>
    : content;
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  scrollView: {
    flex: 1,
  },
  scrollContent: {
    padding: 16,
  },
  markdown: {
    flex: 1,
  },
  highlight: {
    backgroundColor: 'rgba(255, 200, 0, 0.4)',
    color: undefined,
  },
  searchControlsClip: {
    position: 'absolute',
    right: 20,
    overflow: 'hidden',
  },
  searchControls: {
    alignItems: 'center',
  },
  searchControlButton: {
    width: 44,
    height: 44,
    borderRadius: 22,
    justifyContent: 'center',
    alignItems: 'center',
    marginBottom: 8,
  },
  searchControlButtonText: {
    fontSize: 20,
  },
  matchCount: {
    fontSize: 12,
    fontWeight: '600',
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: 10,
    overflow: 'hidden',
    marginBottom: 8,
  },
  floatingMenuContainer: {
    position: 'absolute',
    right: 20,
    alignItems: 'flex-end',
  },
  menuItemsClip: {
    overflow: 'hidden',
    alignItems: 'flex-end',
  },
  mainMenuButton: {
    width: 56,
    height: 56,
    borderRadius: 28,
    justifyContent: 'center',
    alignItems: 'center',
    shadowColor: '#000',
    shadowOffset: {width: 0, height: 2},
    shadowOpacity: 0.3,
    shadowRadius: 4,
    elevation: 5,
  },
  mainMenuButtonText: {
    fontSize: 28,
    fontWeight: '300',
  },
  menuItemContainer: {
    alignItems: 'center',
    flexDirection: 'row',
    marginBottom: 12,
  },
  menuItem: {
    width: 56,
    height: 56,
    borderRadius: 28,
    justifyContent: 'center',
    alignItems: 'center',
    shadowColor: '#000',
    shadowOffset: {width: 0, height: 2},
    shadowOpacity: 0.3,
    shadowRadius: 4,
    elevation: 5,
  },
  menuItemIcon: {
    fontSize: 22,
  },
  menuItemLabel: {
    marginRight: 12,
    fontSize: 14,
    fontWeight: '500',
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: 4,
    overflow: 'hidden',
  },
  menuItemTouchable: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
  },
  infoButton: {
    position: 'absolute',
    right: 16,
    width: 32,
    height: 32,
    borderRadius: 16,
    justifyContent: 'center',
    alignItems: 'center',
  },
  infoButtonText: {
    fontSize: 18,
  },
});

