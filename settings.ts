import { Platform, Settings as RNSettings } from 'react-native';

import { createMMKV } from 'react-native-mmkv';

// Settings keys (for Settings bundle / user preferences)
export const SettingsKeys = {
  SHOW_FRONT_MATTER: 'showFrontMatter',
  COLOR_MODE: 'colorMode',
  THEME: 'theme',
} as const;

// Storage keys (for app-internal storage, not exposed in Settings)
export const StorageKeys = {
  RECENT_SEARCHES: 'recentSearches',
  DISMISSED_SEASON: 'dismissedSeason',
  DISMISSED_HOLIDAY: 'dismissedHoliday',
  RECENT_FILES: 'recentFiles',
} as const;

type SettingsKey = typeof SettingsKeys[keyof typeof SettingsKeys];
type StorageKey = typeof StorageKeys[keyof typeof StorageKeys];

// All settings keys for watching
const ALL_KEYS = Object.values(SettingsKeys);

// MMKV instance for internal storage (both platforms)
const mmkvStorage = createMMKV({ id: 'markdownr-storage' });

// MMKV instance for Android settings (synced with Settings bundle concept)
const mmkvSettingsStorage = Platform.OS === 'android' ? createMMKV({ id: 'markdownr-settings' }) : null;

/**
 * Add a listener for settings changes.
 * - iOS: Uses watchKeys (native notification)
 * - Android: Listens for MMKV changes (for in-app settings)
 * Returns a cleanup function to remove the listener.
 *
 * TODO: iOS watchKeys callback is not firing when returning from Settings app.
 * Need to investigate why - works in other RN 0.81.5 apps.
 */
export function addSettingsListener(callback: () => void): () => void {
  if (Platform.OS === 'ios') {
    // Watch each key individually
    const watchIds = ALL_KEYS.map(key =>
      RNSettings.watchKeys(key, callback)
    );
    return () => watchIds.forEach(id => RNSettings.clearWatch(id));
  } else if (Platform.OS === 'android' && mmkvSettingsStorage) {
    const listener = mmkvSettingsStorage.addOnValueChangedListener((key) => {
      if (ALL_KEYS.includes(key as SettingsKey)) {
        callback();
      }
    });
    return () => listener.remove();
  }
  return () => {};
}

/**
 * Platform-agnostic settings storage (for user preferences).
 * - iOS: Uses UserDefaults via RNSettings (synced with Settings bundle)
 * - Android: Uses MMKV
 */
export const Settings = {
  getBoolean(key: SettingsKey, defaultValue: boolean = false): boolean {
    if (Platform.OS === 'ios') {
      const value = RNSettings.get(key);
      return value !== undefined && value !== null ? Boolean(value) : defaultValue;
    } else {
      return mmkvSettingsStorage?.getBoolean(key) ?? defaultValue;
    }
  },

  getString(key: SettingsKey, defaultValue: string = ''): string {
    if (Platform.OS === 'ios') {
      const value = RNSettings.get(key);
      return value !== undefined && value !== null ? String(value) : defaultValue;
    } else {
      return mmkvSettingsStorage?.getString(key) ?? defaultValue;
    }
  },

  setBoolean(key: SettingsKey, value: boolean): void {
    if (Platform.OS === 'ios') {
      RNSettings.set({ [key]: value });
    } else {
      mmkvSettingsStorage?.set(key, value);
    }
  },

  setString(key: SettingsKey, value: string): void {
    if (Platform.OS === 'ios') {
      RNSettings.set({ [key]: value });
    } else {
      mmkvSettingsStorage?.set(key, value);
    }
  },
};

/**
 * Internal app storage (not exposed in Settings bundle).
 * Uses MMKV on both platforms for fast key-value storage.
 */
export const Storage = {
  getString(key: StorageKey, defaultValue: string = ''): string {
    return mmkvStorage.getString(key) ?? defaultValue;
  },

  setString(key: StorageKey, value: string): void {
    mmkvStorage.set(key, value);
  },

  getStringArray(key: StorageKey, defaultValue: string[] = []): string[] {
    const value = mmkvStorage.getString(key);
    if (!value) return defaultValue;
    try { return JSON.parse(value); } catch { return defaultValue; }
  },

  setStringArray(key: StorageKey, value: string[]): void {
    mmkvStorage.set(key, JSON.stringify(value));
  },
};
