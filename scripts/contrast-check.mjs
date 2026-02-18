#!/usr/bin/env node
// WCAG AA Contrast Ratio Checker for MarkdownrExpo themes
// Run: node scripts/contrast-check.mjs
// Exits with code 1 if any check fails (suitable for CI/pre-commit hooks)

// ---- sRGB to linear conversion ----
function sRGBtoLinear(c) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

function hexToRGB(hex) {
  hex = hex.replace('#', '');
  if (hex.length === 3) hex = hex[0]+hex[0]+hex[1]+hex[1]+hex[2]+hex[2];
  return {
    r: parseInt(hex.substring(0, 2), 16),
    g: parseInt(hex.substring(2, 4), 16),
    b: parseInt(hex.substring(4, 6), 16),
  };
}

function relativeLuminance(hex) {
  const {r, g, b} = hexToRGB(hex);
  const R = sRGBtoLinear(r);
  const G = sRGBtoLinear(g);
  const B = sRGBtoLinear(b);
  return 0.2126 * R + 0.7152 * G + 0.0722 * B;
}

function contrastRatio(hex1, hex2) {
  const L1 = relativeLuminance(hex1);
  const L2 = relativeLuminance(hex2);
  const lighter = Math.max(L1, L2);
  const darker = Math.min(L1, L2);
  return (lighter + 0.05) / (darker + 0.05);
}

// ---- Base themes (from source) ----
const defaultMarkdownThemeColors = {
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
};

const lightMarkdownThemeColors = { ...defaultMarkdownThemeColors };

const darkMarkdownThemeColors = {
  text: '#e2e8f0',
  textMuted: '#94a3b8',
  heading: '#f1f5f9',
  link: '#60a5fa',
  code: '#e2e8f0',
  codeBackground: '#1e293b',
  codeLanguage: '#64748b',
  blockquote: '#334155',
  border: '#334155',
  surface: '#1e293b',
  surfaceLight: '#0f172a',
  accent: '#60a5fa',
  tableBorder: '#334155',
  tableHeader: '#1e293b',
  tableHeaderText: '#94a3b8',
  tableRowEven: 'transparent',
  tableRowOdd: '#1e293b',
};

// ---- All themes (must match themes.ts) ----
const customThemes = {
  default: {
    light: { ...lightMarkdownThemeColors, surface: '#f5f5f5' },
    dark: { ...darkMarkdownThemeColors, surface: '#2a2a2a' },
    background: { light: '#ffffff', dark: '#1a1a1a' },
  },
  ocean: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#1e3a5f', textMuted: '#4a6a8f', heading: '#0c2340',
      link: '#0077b6', code: '#007aa9',
      codeBackground: '#e0f4ff', blockquote: '#0096c7', accent: '#48cae4',
      border: '#90e0ef', tableBorder: '#90e0ef', tableHeader: '#caf0f8',
      tableHeaderText: '#03045e', tableRowEven: '#ffffff', tableRowOdd: '#f0faff',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#caf0f8', textMuted: '#90c0d8', heading: '#90e0ef',
      link: '#48cae4', code: '#00b4d8',
      codeBackground: '#023e8a', blockquote: '#0096c7', accent: '#48cae4',
      border: '#0077b6', surface: '#03045e', surfaceLight: '#023e8a',
      tableBorder: '#0077b6', tableHeader: '#023e8a', tableHeaderText: '#90e0ef',
      tableRowEven: '#03045e', tableRowOdd: '#0a1628',
    },
    background: { light: '#f0faff', dark: '#03045e' },
  },
  forest: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#2d3b2d', textMuted: '#5a6b5a', heading: '#1a2e1a',
      link: '#2d6a4f', code: '#398060',
      codeBackground: '#d8f3dc', blockquote: '#52b788', accent: '#74c69d',
      border: '#95d5b2', tableBorder: '#95d5b2', tableHeader: '#d8f3dc',
      tableHeaderText: '#1b4332', tableRowEven: '#ffffff', tableRowOdd: '#f0fff4',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#d8f3dc', textMuted: '#a0c3ac', heading: '#95d5b2',
      link: '#74c69d', code: '#52b788',
      codeBackground: '#1b4332', blockquote: '#40916c', accent: '#74c69d',
      border: '#2d6a4f', surface: '#081c15', surfaceLight: '#1b4332',
      tableBorder: '#2d6a4f', tableHeader: '#1b4332', tableHeaderText: '#95d5b2',
      tableRowEven: '#081c15', tableRowOdd: '#0d2818',
    },
    background: { light: '#f0fff4', dark: '#081c15' },
  },
  sunset: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#5c3d2e', textMuted: '#8b6b5a', heading: '#3d2314',
      link: '#c74827', code: '#b25a0d',
      codeBackground: '#ffeee6', blockquote: '#e07a5f', accent: '#f2994a',
      border: '#ffcdb2', tableBorder: '#ffcdb2', tableHeader: '#ffe5d9',
      tableHeaderText: '#6d4c41', tableRowEven: '#ffffff', tableRowOdd: '#fff8f5',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#ffcdb2', textMuted: '#c9a090', heading: '#ffb4a2',
      link: '#e5989b', code: '#f2994a',
      codeBackground: '#5c3d2e', blockquote: '#e07a5f', accent: '#f2994a',
      border: '#6d4c41', surface: '#3d2314', surfaceLight: '#5c3d2e',
      tableBorder: '#6d4c41', tableHeader: '#5c3d2e', tableHeaderText: '#ffcdb2',
      tableRowEven: '#3d2314', tableRowOdd: '#4a2c1a',
    },
    background: { light: '#fff8f5', dark: '#3d2314' },
  },
  lavender: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#4a4063', textMuted: '#756b8e', heading: '#2d2640',
      link: '#7c3aed', code: '#7a4ff7',
      codeBackground: '#ede9fe', blockquote: '#8b5cf6', accent: '#a78bfa',
      border: '#c4b5fd', tableBorder: '#c4b5fd', tableHeader: '#ede9fe',
      tableHeaderText: '#5b21b6', tableRowEven: '#ffffff', tableRowOdd: '#faf5ff',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#e9d5ff', textMuted: '#b9a5cf', heading: '#c4b5fd',
      link: '#a78bfa', code: '#c084fc',
      codeBackground: '#3b0764', blockquote: '#8b5cf6', accent: '#a78bfa',
      border: '#6b21a8', surface: '#2e1065', surfaceLight: '#3b0764',
      tableBorder: '#6b21a8', tableHeader: '#3b0764', tableHeaderText: '#c4b5fd',
      tableRowEven: '#2e1065', tableRowOdd: '#1e0a3e',
    },
    background: { light: '#faf5ff', dark: '#2e1065' },
  },
  winter: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#2c3e50', textMuted: '#657281', heading: '#1a252f',
      link: '#2e75b5', code: '#2c75b6',
      codeBackground: '#e8f0fe', blockquote: '#a4c2f4', accent: '#5b9bd5',
      border: '#c6d9f1', tableBorder: '#c6d9f1', tableHeader: '#e8f0fe',
      tableHeaderText: '#2c3e50', tableRowEven: '#ffffff', tableRowOdd: '#f4f8ff',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#cfe2f3', textMuted: '#8eaec7', heading: '#a4c2f4',
      link: '#6fa8dc', code: '#5b9bd5',
      codeBackground: '#1a2a3e', blockquote: '#3d6b99', accent: '#6fa8dc',
      border: '#2c4a6e', surface: '#0d1b2a', surfaceLight: '#1a2a3e',
      tableBorder: '#2c4a6e', tableHeader: '#1a2a3e', tableHeaderText: '#a4c2f4',
      tableRowEven: '#0d1b2a', tableRowOdd: '#112233',
    },
    background: { light: '#f4f8ff', dark: '#0d1b2a' },
  },
  spring: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#3b4f2a', textMuted: '#5e7a4b', heading: '#2a3d1a',
      link: '#db1f57', code: '#be496a',
      codeBackground: '#fce4ec', blockquote: '#f48fb1', accent: '#e75480',
      border: '#f8bbd0', tableBorder: '#f8bbd0', tableHeader: '#fce4ec',
      tableHeaderText: '#880e4f', tableRowEven: '#ffffff', tableRowOdd: '#fef7f9',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#f8d7e3', textMuted: '#c9a0b2', heading: '#f48fb1',
      link: '#f06292', code: '#e75480',
      codeBackground: '#4a1a2e', blockquote: '#c2185b', accent: '#f06292',
      border: '#6a1b3d', surface: '#2a0e1e', surfaceLight: '#4a1a2e',
      tableBorder: '#6a1b3d', tableHeader: '#4a1a2e', tableHeaderText: '#f48fb1',
      tableRowEven: '#2a0e1e', tableRowOdd: '#351428',
    },
    background: { light: '#fef7f9', dark: '#2a0e1e' },
  },
  summer: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#5c4a1e', textMuted: '#86723e', heading: '#3d3010',
      link: '#956d0f', code: '#936e0c',
      codeBackground: '#fff8e1', blockquote: '#ffc107', accent: '#ffb300',
      border: '#ffe082', tableBorder: '#ffe082', tableHeader: '#fff8e1',
      tableHeaderText: '#5c4a1e', tableRowEven: '#ffffff', tableRowOdd: '#fffcf0',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#ffe082', textMuted: '#c4a44a', heading: '#ffd54f',
      link: '#ffca28', code: '#ffb300',
      codeBackground: '#3e2c06', blockquote: '#f9a825', accent: '#ffca28',
      border: '#5c4a1e', surface: '#261c04', surfaceLight: '#3e2c06',
      tableBorder: '#5c4a1e', tableHeader: '#3e2c06', tableHeaderText: '#ffd54f',
      tableRowEven: '#261c04', tableRowOdd: '#302208',
    },
    background: { light: '#fffcf0', dark: '#261c04' },
  },
  autumn: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#5d3a1a', textMuted: '#8b6942', heading: '#3e2410',
      link: '#b9511e', code: '#b0581a',
      codeBackground: '#fbe9da', blockquote: '#e65100', accent: '#f57c00',
      border: '#ffcc80', tableBorder: '#ffcc80', tableHeader: '#fbe9da',
      tableHeaderText: '#5d3a1a', tableRowEven: '#ffffff', tableRowOdd: '#fdf5ee',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#ffcc80', textMuted: '#c49050', heading: '#ffb74d',
      link: '#ff9800', code: '#f57c00',
      codeBackground: '#3e2010', blockquote: '#e65100', accent: '#ff9800',
      border: '#5d3a1a', surface: '#2a1508', surfaceLight: '#3e2010',
      tableBorder: '#5d3a1a', tableHeader: '#3e2010', tableHeaderText: '#ffb74d',
      tableRowEven: '#2a1508', tableRowOdd: '#331a0c',
    },
    background: { light: '#fdf5ee', dark: '#2a1508' },
  },
  christmas: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#2e4033', textMuted: '#59785e', heading: '#b71c1c',
      link: '#c62828', code: '#2e7d32',
      codeBackground: '#e8f5e9', blockquote: '#c62828', accent: '#d32f2f',
      border: '#c8e6c9', tableBorder: '#c8e6c9', tableHeader: '#ffebee',
      tableHeaderText: '#b71c1c', tableRowEven: '#ffffff', tableRowOdd: '#f1f8f2',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#c8e6c9', textMuted: '#81a784', heading: '#ef5350',
      link: '#ef9a9a', code: '#81c784',
      codeBackground: '#1b3a1e', blockquote: '#c62828', accent: '#ef5350',
      border: '#2e5930', surface: '#0d1f10', surfaceLight: '#1b3a1e',
      tableBorder: '#2e5930', tableHeader: '#2a1010', tableHeaderText: '#ef9a9a',
      tableRowEven: '#0d1f10', tableRowOdd: '#142818',
    },
    background: { light: '#f1f8f2', dark: '#0d1f10' },
  },
  easter: {
    light: {
      ...lightMarkdownThemeColors,
      text: '#4a3f5c', textMuted: '#766b87', heading: '#6a1b9a',
      link: '#a944bb', code: '#557a2d',
      codeBackground: '#f3e5f5', blockquote: '#ce93d8', accent: '#ab47bc',
      border: '#e1bee7', tableBorder: '#e1bee7', tableHeader: '#f3e5f5',
      tableHeaderText: '#6a1b9a', tableRowEven: '#ffffff', tableRowOdd: '#fdf2ff',
    },
    dark: {
      ...darkMarkdownThemeColors,
      text: '#e1bee7', textMuted: '#b088b8', heading: '#ce93d8',
      link: '#ba68c8', code: '#aed581',
      codeBackground: '#301540', blockquote: '#8e24aa', accent: '#ba68c8',
      border: '#4a1860', surface: '#1a0a28', surfaceLight: '#301540',
      tableBorder: '#4a1860', tableHeader: '#301540', tableHeaderText: '#ce93d8',
      tableRowEven: '#1a0a28', tableRowOdd: '#221032',
    },
    background: { light: '#fdf2ff', dark: '#1a0a28' },
  },
};

// ---- Check pairs ----
// Normal text (4.5:1): text, textMuted, link, code
// Large text (3:1): heading

const colorPairs = [
  { key: 'text',      minRatio: 4.5, label: 'normal text' },
  { key: 'textMuted', minRatio: 4.5, label: 'normal text' },
  { key: 'link',      minRatio: 4.5, label: 'normal text' },
  { key: 'code',      minRatio: 4.5, label: 'normal text' },
  { key: 'heading',   minRatio: 3.0, label: 'large text (heading)' },
];

let failCount = 0;
let passCount = 0;
const failures = [];

for (const [themeName, theme] of Object.entries(customThemes)) {
  for (const variant of ['light', 'dark']) {
    const bg = theme.background[variant];
    const colors = theme[variant];

    for (const pair of colorPairs) {
      const fg = colors[pair.key];
      if (!fg) continue;

      const ratio = contrastRatio(fg, bg);
      const pass = ratio >= pair.minRatio;

      if (pass) {
        passCount++;
      } else {
        failCount++;
        failures.push({
          theme: themeName,
          variant,
          property: pair.key,
          fg,
          bg,
          ratio: ratio.toFixed(2),
          required: pair.minRatio,
          label: pair.label,
        });
      }
    }
  }
}

console.log('=== WCAG AA Contrast Ratio Audit ===\n');
console.log(`Total checks: ${passCount + failCount}`);
console.log(`Passing: ${passCount}`);
console.log(`Failing: ${failCount}\n`);

if (failures.length > 0) {
  console.log('=== FAILURES ===\n');
  for (const f of failures) {
    console.log(`FAIL: ${f.theme} (${f.variant}) - ${f.property}`);
    console.log(`  Foreground: ${f.fg}  |  Background: ${f.bg}`);
    console.log(`  Ratio: ${f.ratio}:1  (required: ${f.required}:1 for ${f.label})`);
    console.log('');
  }
  process.exit(1);
} else {
  console.log('All color pairs pass WCAG AA requirements!');
}
