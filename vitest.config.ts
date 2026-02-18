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
    },
  },
});
