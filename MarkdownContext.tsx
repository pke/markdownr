import React from 'react';
import {View, ScrollView} from 'react-native';
// @ts-ignore - importing from source to use patched astTransform
import {defaultMarkdownTheme, type MarkdownTheme} from 'react-native-nitro-markdown/src';
import {EXAMPLE_MARKDOWN} from './example';
import type {ThemeName, ThemeConfig} from './themes';

export type TocHeading = {
  index: number;
  level: number;
  text: string;
};

export type SearchMatch = {
  line: number;
  charPosition: number;
  percent: number;
};

export type MarkdownContextType = {
  markdownContent: string;
  setMarkdownContent: (content: string) => void;
  fileName: string | null;
  setFileName: (name: string | null) => void;
  scrollToPercent: number | null;
  setScrollToPercent: (percent: number | null) => void;
  highlightText: string | null;
  setHighlightText: (text: string | null) => void;
  searchMatches: SearchMatch[];
  setSearchMatches: (matches: SearchMatch[]) => void;
  currentMatchIndex: number;
  setCurrentMatchIndex: (index: number) => void;
  theme: MarkdownTheme;
  backgroundColor: string;
  isDarkMode: boolean;
  toggleDarkMode: () => void;
  themeName: ThemeName;
  cycleTheme: () => void;
  showFrontMatterSetting: boolean;
  tocHeadingsRef: React.RefObject<TocHeading[]>;
  scrollToHeadingIndex: number | null;
  setScrollToHeadingIndex: (index: number | null) => void;
  headingRefsMap: React.RefObject<Map<number, View>>;
  scrollViewRef: React.RefObject<ScrollView | null>;
  frontMatterTheme: ThemeConfig | null;
  setFrontMatterTheme: (theme: ThemeConfig | null) => void;
  frontMatterThemeApplied: boolean;
  setFrontMatterThemeApplied: (applied: boolean) => void;
  applyTheme: (name: ThemeName) => void;
};

const emptyMap = new Map<number, View>();
const emptyRef = {current: emptyMap};
const nullScrollRef = {current: null};

export const MarkdownContext = React.createContext<MarkdownContextType>({
  markdownContent: EXAMPLE_MARKDOWN,
  setMarkdownContent: () => {},
  fileName: null,
  setFileName: () => {},
  scrollToPercent: null,
  setScrollToPercent: () => {},
  highlightText: null,
  setHighlightText: () => {},
  searchMatches: [],
  setSearchMatches: () => {},
  currentMatchIndex: 0,
  setCurrentMatchIndex: () => {},
  theme: defaultMarkdownTheme,
  backgroundColor: '#1a1a1a',
  isDarkMode: true,
  toggleDarkMode: () => {},
  themeName: 'default',
  cycleTheme: () => {},
  showFrontMatterSetting: false,
  tocHeadingsRef: {current: []},
  scrollToHeadingIndex: null,
  setScrollToHeadingIndex: () => {},
  headingRefsMap: emptyRef,
  scrollViewRef: nullScrollRef,
  frontMatterTheme: null,
  setFrontMatterTheme: () => {},
  frontMatterThemeApplied: false,
  setFrontMatterThemeApplied: () => {},
  applyTheme: () => {},
});
