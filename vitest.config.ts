import {defineConfig} from 'vitest/config';
import path from 'path';

export default defineConfig({
  test: {
    include: ['__tests__/**/*.test.ts'],
  },
  resolve: {
    alias: {
      'react-native-nitro-markdown/src': path.resolve(__dirname, '__tests__/mocks/react-native-nitro-markdown.ts'),
      'react-native-nitro-markdown': path.resolve(__dirname, '__tests__/mocks/react-native-nitro-markdown.ts'),
      'react-native': path.resolve(__dirname, '__tests__/mocks/react-native.ts'),
      'expo-file-system/next': path.resolve(__dirname, '__tests__/mocks/expo-file-system-next.ts'),
      'react-native-mmkv': path.resolve(__dirname, '__tests__/mocks/react-native-mmkv.ts'),
      'expo-localization': path.resolve(__dirname, '__tests__/mocks/expo-localization.ts'),
      'folder-picker': path.resolve(__dirname, '__tests__/mocks/folder-picker.ts'),
      'expo-document-picker': path.resolve(__dirname, '__tests__/mocks/expo-document-picker.ts'),
      'expo-font': path.resolve(__dirname, '__tests__/mocks/expo-font.ts'),
    },
  },
});
