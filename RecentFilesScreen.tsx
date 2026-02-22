import React, {useState, useEffect, useCallback} from 'react';
import {
  StyleSheet,
  View,
  Text,
  TouchableOpacity,
  Alert,
  ScrollView,
} from 'react-native';
import Animated, {FadeOut, LinearTransition} from 'react-native-reanimated';
import {useNavigation} from '@react-navigation/native';
import {useSafeAreaInsets} from 'react-native-safe-area-context';
import {GestureHandlerRootView} from 'react-native-gesture-handler';
import ReanimatedSwipeable from 'react-native-gesture-handler/ReanimatedSwipeable';

import {MarkdownContext} from './MarkdownContext';
import {getRecentFiles, loadRecentFile, deleteRecentFile, clearAllRecentFiles, type RecentFileEntry} from './recentFiles';

export function RecentFilesScreen() {
  const navigation = useNavigation();
  const insets = useSafeAreaInsets();
  const {
    openFile,
    theme,
    backgroundColor,
  } = React.useContext(MarkdownContext);

  const [recentFiles, setRecentFiles] = useState<RecentFileEntry[]>([]);

  useEffect(() => {
    setRecentFiles(getRecentFiles());
  }, []);

  const handleOpenRecentFile = useCallback(async (entry: RecentFileEntry) => {
    const content = await loadRecentFile(entry);
    if (content) {
      openFile(content, entry.subtitle);
      navigation.goBack();
    } else {
      Alert.alert('File unavailable', 'This file was removed by iOS to free up storage space. Open the original file again to re-cache it.');
      setRecentFiles(prev => prev.filter(e => e.id !== entry.id));
      deleteRecentFile(entry.id);
    }
  }, [openFile, navigation]);

  const handleDeleteRecentFile = useCallback((id: string) => {
    setRecentFiles(prev => prev.filter(e => e.id !== id));
    deleteRecentFile(id);
  }, []);

  const handleClearAll = useCallback(() => {
    setRecentFiles([]);
    clearAllRecentFiles();
  }, []);

  return (
    <GestureHandlerRootView style={styles.container}>
      <View style={[styles.container, {backgroundColor}]}>
        <View style={[styles.header, {paddingTop: insets.top + 8}]}>
          <Text style={[styles.title, {color: theme.colors.heading}]}>Recent Files</Text>
          <View style={styles.headerActions}>
            {recentFiles.length > 0 && (
              <TouchableOpacity onPress={handleClearAll} activeOpacity={0.7}>
                <Text style={[styles.clearAll, {color: theme.colors.link}]}>Clear All</Text>
              </TouchableOpacity>
            )}
            <TouchableOpacity onPress={() => navigation.goBack()} activeOpacity={0.7}>
              <Text style={[styles.doneButton, {color: theme.colors.link}]}>Done</Text>
            </TouchableOpacity>
          </View>
        </View>
        <ScrollView style={styles.scrollView} contentContainerStyle={styles.scrollContent}>
          {recentFiles.length === 0 && (
            <Text style={[styles.emptyText, {color: theme.colors.textMuted}]}>
              No recent files. Open a markdown file to see it here.
            </Text>
          )}
          {recentFiles.map((entry) => (
            <Animated.View
              key={entry.id}
              exiting={FadeOut.duration(200)}
              layout={LinearTransition.duration(200)}>
              <ReanimatedSwipeable
                friction={2}
                rightThreshold={40}
                renderRightActions={() => (
                  <TouchableOpacity
                    style={styles.deleteAction}
                    onPress={() => handleDeleteRecentFile(entry.id)}
                    activeOpacity={0.7}>
                    <Text style={styles.deleteActionText}>Delete</Text>
                  </TouchableOpacity>
                )}>
                <TouchableOpacity
                  style={[styles.item, {backgroundColor, borderBottomColor: theme.colors.border}]}
                  onPress={() => handleOpenRecentFile(entry)}
                  activeOpacity={0.7}>
                  <Text style={[styles.itemIcon, {color: theme.colors.textMuted}]}>📄</Text>
                  <View style={styles.itemContent}>
                    <Text style={[styles.itemTitle, {color: theme.colors.heading}]} numberOfLines={1}>
                      {entry.title}
                    </Text>
                    <Text style={[styles.itemSubtitle, {color: theme.colors.textMuted}]} numberOfLines={1}>
                      {entry.subtitle}
                    </Text>
                  </View>
                </TouchableOpacity>
              </ReanimatedSwipeable>
            </Animated.View>
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
  deleteAction: {
    backgroundColor: '#ff3b30',
    justifyContent: 'center',
    alignItems: 'center',
    width: 80,
  },
  deleteActionText: {
    color: '#fff',
    fontWeight: '600',
    fontSize: 14,
  },
});
