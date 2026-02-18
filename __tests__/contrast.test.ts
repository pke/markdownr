import {describe, it, expect} from 'vitest';
import {customThemes} from '../themes';

// --- sRGB / WCAG helpers ---
function sRGBtoLinear(c: number) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

function hexToRGB(hex: string) {
  hex = hex.replace('#', '');
  if (hex.length === 3) hex = hex[0]+hex[0]+hex[1]+hex[1]+hex[2]+hex[2];
  return {
    r: parseInt(hex.substring(0, 2), 16),
    g: parseInt(hex.substring(2, 4), 16),
    b: parseInt(hex.substring(4, 6), 16),
  };
}

function relativeLuminance(hex: string) {
  const {r, g, b} = hexToRGB(hex);
  return 0.2126 * sRGBtoLinear(r) + 0.7152 * sRGBtoLinear(g) + 0.0722 * sRGBtoLinear(b);
}

function contrastRatio(hex1: string, hex2: string) {
  const L1 = relativeLuminance(hex1);
  const L2 = relativeLuminance(hex2);
  return (Math.max(L1, L2) + 0.05) / (Math.min(L1, L2) + 0.05);
}

// --- Color pairs to check ---
const colorPairs = [
  {key: 'text',      minRatio: 4.5, label: 'normal text'},
  {key: 'textMuted', minRatio: 4.5, label: 'normal text'},
  {key: 'link',      minRatio: 4.5, label: 'normal text'},
  {key: 'code',      minRatio: 4.5, label: 'normal text'},
  {key: 'heading',   minRatio: 3.0, label: 'large text (heading)'},
] as const;

// --- Tests ---
describe('WCAG AA contrast', () => {
  for (const [themeName, themeConfig] of Object.entries(customThemes)) {
    for (const variant of ['light', 'dark'] as const) {
      const bg = themeConfig.background[variant];
      const colors = themeConfig[variant].colors;

      for (const pair of colorPairs) {
        const fg = colors[pair.key];
        if (!fg || fg === 'transparent') continue;

        it(`${themeName} ${variant} — ${pair.key} vs background (${pair.label}, min ${pair.minRatio}:1)`, () => {
          const ratio = contrastRatio(fg, bg);
          expect(ratio, `${fg} on ${bg} = ${ratio.toFixed(2)}:1`).toBeGreaterThanOrEqual(pair.minRatio);
        });
      }
    }
  }
});
