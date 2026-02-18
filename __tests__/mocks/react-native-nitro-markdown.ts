export const defaultMarkdownTheme = {
  colors: {
    text: '#0f172a',
    textMuted: '#64748b',
    heading: '#0f172a',
    link: '#2563eb',
    code: '#0f172a',
    codeBackground: '#f1f5f9',
    codeLanguage: '#94a3b8',
    blockquote: '#cbd5f5',
    border: '#e2e8f0',
    surface: '#ffffff',
    surfaceLight: '#f8fafc',
    accent: '#2563eb',
    tableBorder: '#e2e8f0',
    tableHeader: '#f8fafc',
    tableHeaderText: '#64748b',
    tableRowEven: 'transparent',
    tableRowOdd: '#f8fafc',
  },
  spacing: {xs: 4, s: 8, m: 12, l: 16, xl: 24},
  fontSizes: {xs: 12, s: 14, m: 16, l: 18, xl: 22, h1: 32, h2: 26, h3: 22, h4: 18, h5: 16, h6: 14},
  fontFamilies: {regular: undefined, heading: undefined, mono: 'monospace'},
  headingWeight: undefined,
  borderRadius: {s: 6, m: 10, l: 14},
  showCodeLanguage: false,
};

export type MarkdownTheme = typeof defaultMarkdownTheme;
