import React, {useState, useEffect, useCallback, useRef} from 'react';
import {
  StyleSheet,
  View,
  ScrollView,
  Text,
  TouchableOpacity,
  Keyboard,
} from 'react-native';
import {TouchableOpacity as GHTouchableOpacity} from 'react-native-gesture-handler';
import Animated, {FadeOut, LinearTransition} from 'react-native-reanimated';
import ReanimatedSwipeable from 'react-native-gesture-handler/ReanimatedSwipeable';
import {GestureHandlerRootView} from 'react-native-gesture-handler';
import {useNavigation} from '@react-navigation/native';
import {useTranslation} from 'react-i18next';
import {MarkdownContext} from './MarkdownContext';
import {Storage, StorageKeys} from './settings';

export function SearchScreen() {
  const {t} = useTranslation();
  const navigation = useNavigation();
  const {markdownContent, setScrollToPercent, setHighlightText, setSearchMatches, setCurrentMatchIndex, theme, backgroundColor} = React.useContext(MarkdownContext);
  const [searchQuery, setSearchQuery] = useState('');
  const [results, setResults] = useState<{line: number; text: string; charPosition: number; percent: number}[]>([]);
  const [recentSearches, setRecentSearches] = useState<string[]>([]);
  const borderColor = theme.colors.border ?? '#444';

  // Use refs so the native search bar callbacks always access latest values
  const searchQueryRef = useRef('');
  searchQueryRef.current = searchQuery;

  const onSubmitRef = useRef<(text: string) => void>(() => {});
  onSubmitRef.current = (text: string) => {
    // Fall back to current searchQuery state if text is empty/invalid
    const query = (text && text.length > 0) ? text : searchQueryRef.current;
    if (!query || query.length === 0) return;
    const lines = markdownContent.split('\n');
    let charPos = 0;
    const matches: {line: number; text: string; charPosition: number; percent: number}[] = [];
    lines.forEach((line, index) => {
      if (line.toLowerCase().includes(query.toLowerCase())) {
        matches.push({line: index + 1, text: line, charPosition: charPos, percent: charPos / markdownContent.length});
      }
      charPos += line.length + 1;
    });
    if (matches.length === 0) {
      // No results — stay on search screen so user sees "No results found"
      setSearchQuery(query);
      return;
    }
    setSearchMatches(matches.map(m => ({line: m.line, charPosition: m.charPosition, percent: m.percent})));
    setHighlightText(query);
    setCurrentMatchIndex(0);
    setScrollToPercent(matches[0].percent);
    // Save to recent searches
    const saved = Storage.getStringArray(StorageKeys.RECENT_SEARCHES, []);
    const updated = [query, ...saved.filter(s => s !== query)].slice(0, 10);
    Storage.setStringArray(StorageKeys.RECENT_SEARCHES, updated);
    navigation.goBack();
  };

  // Configure native search bar — only set once on mount
  React.useLayoutEffect(() => {
    navigation.setOptions({
      headerShown: true,
      headerTitle: t('search.title'),
      headerStyle: {backgroundColor},
      headerTintColor: theme.colors.text,
      headerSearchBarOptions: {
        placeholder: t('search.placeholder'),
        autoCapitalize: 'none' as const,
        hideWhenScrolling: false,
        tintColor: theme.colors.link,
        textColor: theme.colors.text,
        onChangeText: (text: string) => setSearchQuery(text),
        onCancelButtonPress: () => navigation.goBack(),
        onSearchButtonPress: (e: any) => {
          // v7 passes string, v6 passes event — handle both
          const text = typeof e === 'string' ? e : e?.nativeEvent?.text ?? '';
          onSubmitRef.current(text);
        },
      },
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [navigation]);

  // Load recent searches on mount
  useEffect(() => {
    const saved = Storage.getStringArray(StorageKeys.RECENT_SEARCHES, []);
    setRecentSearches(saved);
  }, []);

  // Save search to recent searches
  const saveToRecentSearches = useCallback((query: string) => {
    if (!query.trim()) return;
    const updated = [query, ...recentSearches.filter(s => s !== query)].slice(0, 10);
    setRecentSearches(updated);
    Storage.setStringArray(StorageKeys.RECENT_SEARCHES, updated);
  }, [recentSearches]);

  // Delete a single recent search
  const deleteRecentSearch = useCallback((query: string) => {
    const updated = recentSearches.filter(s => s !== query);
    setRecentSearches(updated);
    Storage.setStringArray(StorageKeys.RECENT_SEARCHES, updated);
  }, [recentSearches]);

  // Clear all recent searches
  const clearAllRecentSearches = useCallback(() => {
    setRecentSearches([]);
    Storage.setStringArray(StorageKeys.RECENT_SEARCHES, []);
  }, []);

  const handleResultPress = useCallback((index: number) => {
    Keyboard.dismiss();
    const result = results[index];
    saveToRecentSearches(searchQuery);
    setHighlightText(searchQuery);
    setCurrentMatchIndex(index);
    setScrollToPercent(result.percent);
    navigation.goBack();
  }, [results, searchQuery, saveToRecentSearches, setHighlightText, setCurrentMatchIndex, setScrollToPercent, navigation]);

  useEffect(() => {
    if (searchQuery.length > 0) {
      const lines = markdownContent.split('\n');
      const matches: {line: number; text: string; charPosition: number; percent: number}[] = [];
      let charPos = 0;
      lines.forEach((line, index) => {
        if (line.toLowerCase().includes(searchQuery.toLowerCase())) {
          const percent = charPos / markdownContent.length;
          matches.push({line: index + 1, text: line, charPosition: charPos, percent});
        }
        charPos += line.length + 1;
      });
      setResults(matches);
      setSearchMatches(matches.map(m => ({line: m.line, charPosition: m.charPosition, percent: m.percent})));
    } else {
      setResults([]);
      setSearchMatches([]);
    }
  }, [searchQuery, markdownContent, setSearchMatches]);

  const handleRecentSearchPress = useCallback((query: string) => {
    saveToRecentSearches(query);
    const lines = markdownContent.split('\n');
    let charPos = 0;
    const matches: {line: number; text: string; charPosition: number; percent: number}[] = [];
    lines.forEach((line, index) => {
      if (line.toLowerCase().includes(query.toLowerCase())) {
        matches.push({line: index + 1, text: line, charPosition: charPos, percent: charPos / markdownContent.length});
      }
      charPos += line.length + 1;
    });
    if (matches.length === 0) {
      // No results — update query so "No results found" shows
      setSearchQuery(query);
      return;
    }
    setSearchMatches(matches.map(m => ({line: m.line, charPosition: m.charPosition, percent: m.percent})));
    setHighlightText(query);
    setCurrentMatchIndex(0);
    setScrollToPercent(matches[0].percent);
    navigation.goBack();
  }, [markdownContent, saveToRecentSearches, setHighlightText, setSearchMatches, setCurrentMatchIndex, setScrollToPercent, navigation]);

  return (
    <GestureHandlerRootView style={[styles.container, {backgroundColor}]}>
      <ScrollView
        contentInsetAdjustmentBehavior="automatic"
        keyboardShouldPersistTaps="handled"
        keyboardDismissMode="on-drag"
        style={styles.container}
      >
        {results.length > 0 ? (
          results.map((result, index) => (
            <TouchableOpacity
              key={`${result.line}-${result.charPosition}`}
              style={[styles.searchResult, {borderBottomColor: borderColor}]}
              onPress={() => handleResultPress(index)}
              activeOpacity={0.7}
            >
              <Text style={[styles.searchResultLine, {color: theme.colors.textMuted}]}>
                {t('search.line')} {result.line}
              </Text>
              <Text style={[styles.searchResultText, {color: theme.colors.heading}]} numberOfLines={2}>
                {result.text}
              </Text>
            </TouchableOpacity>
          ))
        ) : searchQuery.length > 0 ? (
          <View style={styles.noResults}>
            <Text style={{color: theme.colors.text}}>{t('search.noResults')}</Text>
          </View>
        ) : recentSearches.length > 0 ? (
          <View style={styles.recentSearches}>
            <View style={styles.recentSearchesHeader}>
              <Text style={[styles.recentSearchesTitle, {color: theme.colors.textMuted}]}>
                {t('search.recentSearches')}
              </Text>
              <TouchableOpacity onPress={clearAllRecentSearches} activeOpacity={0.7}>
                <Text style={[styles.clearAllText, {color: theme.colors.link}]}>
                  {t('common.clearAll')}
                </Text>
              </TouchableOpacity>
            </View>
            {recentSearches.map((query) => (
              <Animated.View
                key={query}
                exiting={FadeOut.duration(200)}
                layout={LinearTransition.duration(200)}
              >
                <ReanimatedSwipeable
                  friction={2}
                  rightThreshold={40}
                  renderRightActions={() => (
                    <TouchableOpacity
                      style={styles.deleteAction}
                      onPress={() => deleteRecentSearch(query)}
                      activeOpacity={0.7}
                    >
                      <Text style={styles.deleteActionText}>{t('common.delete')}</Text>
                    </TouchableOpacity>
                  )}
                >
                  <GHTouchableOpacity
                    style={[styles.recentSearchItem, {backgroundColor, borderBottomColor: borderColor}]}
                    onPress={() => handleRecentSearchPress(query)}
                    activeOpacity={0.7}
                  >
                    <Text style={[styles.recentSearchText, {color: theme.colors.heading}]} numberOfLines={1}>
                      {query}
                    </Text>
                  </GHTouchableOpacity>
                </ReanimatedSwipeable>
              </Animated.View>
            ))}
          </View>
        ) : null}
      </ScrollView>
    </GestureHandlerRootView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  searchResult: {
    paddingHorizontal: 16,
    paddingVertical: 12,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  searchResultLine: {
    fontSize: 12,
    marginBottom: 4,
  },
  searchResultText: {
    fontSize: 14,
  },
  noResults: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 40,
  },
  recentSearches: {
    paddingTop: 8,
    paddingBottom: 16,
  },
  recentSearchesHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingHorizontal: 16,
    paddingBottom: 8,
  },
  recentSearchesTitle: {
    fontSize: 13,
    fontWeight: '600',
    textTransform: 'uppercase',
  },
  clearAllText: {
    fontSize: 14,
  },
  recentSearchItem: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 16,
    paddingVertical: 16,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  recentSearchText: {
    flex: 1,
    fontSize: 17,
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
    fontSize: 15,
  },
});
