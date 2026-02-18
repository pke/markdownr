import React from 'react';
import {StyleSheet, Text, TouchableOpacity, View} from 'react-native';
import {DrawerContentScrollView, type DrawerContentComponentProps} from '@react-navigation/drawer';
import Constants from 'expo-constants';
import {MarkdownContext} from './MarkdownContext';
import {getAboutMarkdown} from './about';
import {ThemeSuggestion} from './ThemeSuggestion';

export function TocDrawerContent({navigation}: DrawerContentComponentProps) {
  const {
    tocHeadingsRef, setScrollToHeadingIndex, setMarkdownContent, setFileName,
    theme, backgroundColor,
  } = React.useContext(MarkdownContext);
  const tocHeadings = tocHeadingsRef.current;

  const handleHeadingPress = (index: number) => {
    setScrollToHeadingIndex(index);
    navigation.closeDrawer();
  };

  const handleAbout = () => {
    setMarkdownContent(getAboutMarkdown());
    setFileName('about.md');
    navigation.closeDrawer();
  };

  return (
    <DrawerContentScrollView style={{backgroundColor}}>
      <ThemeSuggestion variant="drawer" onApply={() => navigation.closeDrawer()} />
      <Text style={[styles.title, {color: theme.colors.heading}]}>Contents</Text>
      {tocHeadings.length === 0 ? (
        <Text style={[styles.emptyText, {color: theme.colors.text}]}>
          No headings found
        </Text>
      ) : (
        tocHeadings.map((heading) => (
          <TouchableOpacity
            key={heading.index}
            style={[styles.item, {paddingLeft: 16 + (heading.level - 1) * 12}]}
            onPress={() => handleHeadingPress(heading.index)}
            activeOpacity={0.7}>
            <Text
              style={[
                styles.itemText,
                {color: theme.colors.text},
                heading.level === 1 && styles.itemTextH1,
                heading.level === 2 && styles.itemTextH2,
              ]}
              numberOfLines={2}>
              {heading.text}
            </Text>
          </TouchableOpacity>
        ))
      )}
      <View style={styles.footerContainer}>
        <TouchableOpacity onPress={handleAbout} activeOpacity={0.7}>
          <Text style={[styles.footerLink, {color: theme.colors.link}]}>About</Text>
        </TouchableOpacity>
        <Text style={[styles.versionText, {color: theme.colors.textMuted}]}>
          Markdownr v{Constants.expoConfig?.version ?? '?'}
        </Text>
      </View>
    </DrawerContentScrollView>
  );
}

const styles = StyleSheet.create({
  title: {
    fontSize: 20,
    fontWeight: '700',
    paddingHorizontal: 16,
    paddingVertical: 16,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: 'rgba(128,128,128,0.3)',
  },
  emptyText: {
    padding: 16,
    fontStyle: 'italic',
    opacity: 0.6,
  },
  item: {
    paddingVertical: 12,
    paddingRight: 16,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: 'rgba(128,128,128,0.2)',
  },
  itemText: {
    fontSize: 15,
  },
  itemTextH1: {
    fontSize: 17,
    fontWeight: '600',
  },
  itemTextH2: {
    fontSize: 16,
    fontWeight: '500',
  },
  footerContainer: {
    paddingTop: 24,
    paddingBottom: 16,
    alignItems: 'center',
    gap: 8,
  },
  footerLink: {
    fontSize: 14,
    fontWeight: '500',
  },
  versionText: {
    fontSize: 12,
    opacity: 0.5,
  },
});
