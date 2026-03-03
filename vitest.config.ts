import {defineConfig} from 'vitest/config';
import path from 'path';

export default defineConfig({
  test: {
    include: ['__tests__/**/*.test.ts'],
  },
  resolve: {
    alias: {
      'react-native-nitro-markdown/src': path.resolve(__dirname, '__tests__/mocks/react-native-nitro-markdown.ts'),
      'react-native': path.resolve(__dirname, '__tests__/mocks/react-native.ts'),
      'expo-file-system/next': path.resolve(__dirname, '__tests__/mocks/expo-file-system-next.ts'),
      'react-native-mmkv': path.resolve(__dirname, '__tests__/mocks/react-native-mmkv.ts'),
      'expo-localization': path.resolve(__dirname, '__tests__/mocks/expo-localization.ts'),
    },
  },
});
