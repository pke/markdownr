import React, {useState, useEffect, useCallback, useRef} from 'react';
import {
  StyleSheet,
  View,
  ScrollView,
  Text,
  TextInput,
  TouchableOpacity,
  Keyboard,
} from 'react-native';
import Animated, {FadeOut, LinearTransition} from 'react-native-reanimated';
import {useNavigation} from '@react-navigation/native';
import {useSafeAreaInsets} from 'react-native-safe-area-context';
import {MarkdownContext} from './MarkdownContext';
import {Storage, StorageKeys} from './settings';

export function SearchScreen() {
  const navigation = useNavigation();
  const insets = useSafeAreaInsets();
  const {markdownContent, setScrollToPercent, setHighlightText, setSearchMatches, setCurrentMatchIndex, theme, backgroundColor, isDarkMode} = React.useContext(MarkdownContext);
  const [searchQuery, setSearchQuery] = useState('');
  const [results, setResults] = useState<{line: number; text: string; charPosition: number; percent: number}[]>([]);
  const [recentSearches, setRecentSearches] = useState<string[]>([]);
  const borderColor = theme.colors.border ?? '#444';
  const inputRef = useRef<TextInput>(null);

  // Hide the default header — we render our own search bar
  React.useLayoutEffect(() => {
    navigation.setOptions({
      headerShown: false,
    });
  }, [navigation]);

  // Auto-focus on mount
  useEffect(() => {
    const timer = setTimeout(() => inputRef.current?.focus(), 100);
    return () => clearTimeout(timer);
  }, []);

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

  const handleSubmit = useCallback(() => {
    if (results.length > 0) {
      handleResultPress(0);
    } else if (searchQuery.length > 0) {
      setHighlightText(searchQuery);
      navigation.goBack();
    }
  }, [results, searchQuery, handleResultPress, setHighlightText, navigation]);

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

  const handleRecentSearchPress = (query: string) => {
    setSearchQuery(query);
    inputRef.current?.focus();
  };

  const inputBgColor = isDarkMode ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.06)';
  const placeholderColor = isDarkMode ? 'rgba(255,255,255,0.4)' : 'rgba(0,0,0,0.35)';

  return (
    <View style={[styles.container, {backgroundColor}]}>
      <View style={[styles.searchHeader, {paddingTop: insets.top + 8}]}>
        <View style={[styles.searchInputContainer, {backgroundColor: inputBgColor}]}>
          <Text style={[styles.searchIcon, {color: placeholderColor}]}>🔍</Text>
          <TextInput
            ref={inputRef}
            style={[styles.searchInput, {color: theme.colors.text}]}
            placeholder="Search in markdown..."
            placeholderTextColor={placeholderColor}
            value={searchQuery}
            onChangeText={setSearchQuery}
            onSubmitEditing={handleSubmit}
            returnKeyType="search"
            autoCorrect={false}
            autoCapitalize="none"
            testID="searchInput"
          />
          {searchQuery.length > 0 && (
            <TouchableOpacity onPress={() => setSearchQuery('')} style={styles.clearButton}>
              <Text style={[styles.clearButtonText, {color: placeholderColor}]}>✕</Text>
            </TouchableOpacity>
          )}
        </View>
        <TouchableOpacity onPress={() => navigation.goBack()} style={styles.cancelButton}>
          <Text style={[styles.cancelButtonText, {color: theme.colors.link}]}>Cancel</Text>
        </TouchableOpacity>
      </View>
      <ScrollView
        style={styles.scrollView}
        keyboardShouldPersistTaps="handled"
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
                Line {result.line}
              </Text>
              <Text style={[styles.searchResultText, {color: theme.colors.heading}]} numberOfLines={2}>
                {result.text}
              </Text>
            </TouchableOpacity>
          ))
        ) : searchQuery.length > 0 ? (
          <View style={styles.noResults}>
            <Text style={{color: theme.colors.text}}>No results found</Text>
          </View>
        ) : recentSearches.length > 0 ? (
          <View style={styles.recentSearches}>
            <View style={styles.recentSearchesHeader}>
              <Text style={[styles.recentSearchesTitle, {color: theme.colors.textMuted}]}>
                Recent Searches
              </Text>
              <TouchableOpacity onPress={clearAllRecentSearches} activeOpacity={0.7}>
                <Text style={[styles.clearAllText, {color: theme.colors.link}]}>
                  Clear All
                </Text>
              </TouchableOpacity>
            </View>
            {recentSearches.map((query) => (
              <Animated.View
                key={query}
                exiting={FadeOut.duration(200)}
                layout={LinearTransition.duration(200)}
              >
                <TouchableOpacity
                  style={[styles.recentSearchItem, {borderBottomColor: borderColor}]}
                  onPress={() => handleRecentSearchPress(query)}
                  activeOpacity={0.7}
                >
                  <Text style={[styles.recentSearchIcon, {color: theme.colors.textMuted}]}>🕒</Text>
                  <Text style={[styles.recentSearchText, {color: theme.colors.heading}]} numberOfLines={1}>
                    {query}
                  </Text>
                  <TouchableOpacity
                    onPress={() => deleteRecentSearch(query)}
                    style={styles.deleteSearchButton}
                    hitSlop={{top: 8, bottom: 8, left: 8, right: 8}}
                    activeOpacity={0.7}
                  >
                    <Text style={[styles.deleteSearchIcon, {color: theme.colors.textMuted}]}>✕</Text>
                  </TouchableOpacity>
                </TouchableOpacity>
              </Animated.View>
            ))}
          </View>
        ) : null}
      </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  searchHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 12,
    paddingBottom: 10,
  },
  searchInputContainer: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    borderRadius: 10,
    paddingHorizontal: 10,
    height: 38,
  },
  searchIcon: {
    fontSize: 14,
    marginRight: 6,
  },
  searchInput: {
    flex: 1,
    fontSize: 16,
    paddingVertical: 0,
  },
  clearButton: {
    padding: 4,
  },
  clearButtonText: {
    fontSize: 14,
  },
  cancelButton: {
    paddingLeft: 12,
  },
  cancelButtonText: {
    fontSize: 16,
  },
  scrollView: {
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
    paddingTop: 16,
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
    paddingVertical: 12,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  recentSearchIcon: {
    fontSize: 14,
    marginRight: 12,
  },
  recentSearchText: {
    flex: 1,
    fontSize: 16,
  },
  deleteSearchButton: {
    padding: 4,
    marginLeft: 8,
  },
  deleteSearchIcon: {
    fontSize: 14,
  },
});
