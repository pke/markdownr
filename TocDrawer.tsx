import React, {useCallback} from 'react';
import {ScrollView, StyleSheet, Text, TouchableOpacity, View} from 'react-native';
import {type DrawerContentComponentProps} from '@react-navigation/drawer';
import {useSafeAreaInsets} from 'react-native-safe-area-context';
import Constants from 'expo-constants';
import {useNavigation} from '@react-navigation/native';
import {MarkdownContext} from './MarkdownContext';
import {getAboutMarkdown} from './about';
import {ThemeSuggestion} from './ThemeSuggestion';
import {getRecentFiles, loadRecentFile, type RecentFileEntry} from './recentFiles';

export function TocDrawerContent({navigation}: DrawerContentComponentProps) {
  const stackNavigation = useNavigation();
  const {
    tocHeadingsRef, setScrollToHeadingIndex, openFile, setMarkdownContent, setFileName,
    theme, backgroundColor,
  } = React.useContext(MarkdownContext);
  const tocHeadings = tocHeadingsRef.current;
  const insets = useSafeAreaInsets();

  const recentFiles = getRecentFiles();

  const handleHeadingPress = (index: number) => {
    setScrollToHeadingIndex(index);
    navigation.closeDrawer();
  };

  const handleOpenRecentFile = useCallback(async (entry: RecentFileEntry) => {
    const content = await loadRecentFile(entry);
    if (content) {
      openFile(content, entry.subtitle);
    }
    navigation.closeDrawer();
  }, [openFile, navigation]);

  const handleAbout = () => {
    setMarkdownContent(getAboutMarkdown());
    setFileName('about.md');
    navigation.closeDrawer();
  };

  return (
    <View style={[styles.container, {backgroundColor, paddingTop: insets.top}]}>
      <ThemeSuggestion variant="drawer" onApply={() => navigation.closeDrawer()} />
      {tocHeadings.length > 0 && (
        <>
          <Text style={[styles.title, {color: theme.colors.heading}]}>Contents</Text>
          <ScrollView style={styles.scrollArea}>
            {tocHeadings.map((heading) => (
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
            ))}
          </ScrollView>
        </>
      )}
      {tocHeadings.length === 0 && <View style={styles.scrollArea} />}
      <View style={[styles.footer, {paddingBottom: insets.bottom + 16}]}>
        {recentFiles.length > 0 && (
          <>
            <Text style={[styles.sectionTitle, {color: theme.colors.heading}]}>Recent Files</Text>
            {recentFiles.slice(0, 2).map((entry) => (
              <TouchableOpacity
                key={entry.id}
                style={styles.recentItem}
                onPress={() => handleOpenRecentFile(entry)}
                activeOpacity={0.7}>
                <Text style={[styles.recentIcon, {color: theme.colors.textMuted}]}>📄</Text>
                <View style={styles.recentContent}>
                  <Text style={[styles.recentTitle, {color: theme.colors.text}]} numberOfLines={1}>
                    {entry.title}
                  </Text>
                  <Text style={[styles.recentSubtitle, {color: theme.colors.textMuted}]} numberOfLines={1}>
                    {entry.subtitle}
                  </Text>
                </View>
              </TouchableOpacity>
            ))}
            {recentFiles.length > 2 && (
              <TouchableOpacity
                style={styles.showAllButton}
                onPress={() => { stackNavigation.navigate('RecentFiles' as never); navigation.closeDrawer(); }}
                activeOpacity={0.7}>
                <Text style={[styles.showAllText, {color: theme.colors.link}]}>
                  Show All ({recentFiles.length})
                </Text>
              </TouchableOpacity>
            )}
          </>
        )}
        <View style={styles.aboutContainer}>
          <TouchableOpacity onPress={handleAbout} activeOpacity={0.7}>
            <Text style={[styles.footerLink, {color: theme.colors.link}]}>About</Text>
          </TouchableOpacity>
          <Text style={[styles.versionText, {color: theme.colors.textMuted}]}>
            Markdownr v{Constants.expoConfig?.version ?? '?'}
          </Text>
        </View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  title: {
    fontSize: 20,
    fontWeight: '700',
    paddingHorizontal: 16,
    paddingVertical: 16,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: 'rgba(128,128,128,0.3)',
  },
  scrollArea: {
    flex: 1,
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
  footer: {
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: 'rgba(128,128,128,0.3)',
  },
  sectionTitle: {
    fontSize: 16,
    fontWeight: '700',
    paddingHorizontal: 16,
    paddingTop: 12,
    paddingBottom: 8,
  },
  recentItem: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 16,
    paddingVertical: 10,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: 'rgba(128,128,128,0.2)',
  },
  recentIcon: {
    fontSize: 18,
    marginRight: 10,
  },
  recentContent: {
    flex: 1,
  },
  recentTitle: {
    fontSize: 14,
    fontWeight: '500',
  },
  recentSubtitle: {
    fontSize: 12,
    marginTop: 1,
  },
  showAllButton: {
    paddingHorizontal: 16,
    paddingVertical: 10,
  },
  showAllText: {
    fontSize: 14,
    fontWeight: '500',
  },
  aboutContainer: {
    paddingTop: 16,
    paddingBottom: 0,
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
