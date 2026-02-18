import {File, Directory, Paths} from 'expo-file-system/next';
import * as Font from 'expo-font';
import {customThemes, type ThemeConfig, type ThemeName} from './themes';

type ResolvedTheme =
  | {type: 'builtin'; name: ThemeName; config: ThemeConfig}
  | {type: 'remote'; url: string};

const allThemeNames = Object.keys(customThemes) as ThemeName[];

export function resolveThemeFromFrontMatter(
  themeValue: string,
): ResolvedTheme | null {
  if (allThemeNames.includes(themeValue as ThemeName)) {
    return {type: 'builtin', name: themeValue as ThemeName, config: customThemes[themeValue as ThemeName]};
  }
  if (themeValue.startsWith('http://') || themeValue.startsWith('https://')) {
    return {type: 'remote', url: themeValue};
  }
  return null;
}

function urlToHash(url: string): string {
  // Simple hash: replace non-alphanumeric chars to create a safe filename
  let hash = 0;
  for (let i = 0; i < url.length; i++) {
    const char = url.charCodeAt(i);
    hash = ((hash << 5) - hash + char) | 0;
  }
  return Math.abs(hash).toString(36);
}

const cacheDir = new Directory(Paths.cache, 'themes');

function ensureCacheDir(): void {
  if (!cacheDir.exists) {
    cacheDir.create({intermediates: true});
  }
}

async function readCachedTheme(url: string): Promise<ThemeConfig | null> {
  try {
    const file = new File(cacheDir, `${urlToHash(url)}.json`);
    if (!file.exists) return null;
    const text = await file.text();
    return validateThemeConfig(JSON.parse(text));
  } catch {
    return null;
  }
}

function writeCachedTheme(url: string, config: ThemeConfig): void {
  try {
    ensureCacheDir();
    const file = new File(cacheDir, `${urlToHash(url)}.json`);
    if (file.exists) file.delete();
    file.create();
    file.write(JSON.stringify(config));
  } catch {
    // Cache write failure is non-fatal
  }
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function isString(v: unknown): v is string {
  return typeof v === 'string';
}

function hasRequiredColors(colors: unknown): boolean {
  if (!isObject(colors)) return false;
  const required = ['text', 'background', 'heading', 'link', 'border', 'codeBackground'];
  return required.every(key => isString(colors[key]));
}

function deepMerge(target: any, source: any): any {
  const result = {...target};
  for (const key of Object.keys(source)) {
    if (isObject(source[key]) && isObject(target[key])) {
      result[key] = deepMerge(target[key], source[key]);
    } else if (source[key] !== undefined) {
      result[key] = source[key];
    }
  }
  return result;
}

export function validateThemeConfig(json: unknown): ThemeConfig | null {
  if (!isObject(json)) return null;

  const {light, dark, background} = json as any;

  // background is required
  if (!isObject(background) || !isString(background.light) || !isString(background.dark)) {
    return null;
  }

  // At least one of light/dark must have colors
  if (!isObject(light) && !isObject(dark)) return null;

  const hasLightColors = isObject(light) && hasRequiredColors(light.colors);
  const hasDarkColors = isObject(dark) && hasRequiredColors(dark.colors);

  if (!hasLightColors && !hasDarkColors) return null;

  // Merge with defaults for missing fields
  const defaults = customThemes.default;
  return {
    icon: isString((json as any).icon) ? (json as any).icon : '🎨',
    author: isString((json as any).author) ? (json as any).author : undefined,
    light: hasLightColors ? deepMerge(defaults.light, light) : defaults.light,
    dark: hasDarkColors ? deepMerge(defaults.dark, dark) : defaults.dark,
    background: background as ThemeConfig['background'],
    overlay: json.overlay as ThemeConfig['overlay'],
    customRenderers: json.customRenderers as ThemeConfig['customRenderers'],
    fonts: json.fonts as ThemeConfig['fonts'],
  };
}

async function loadFonts(
  fonts: NonNullable<ThemeConfig['fonts']>,
): Promise<{regular?: string; heading?: string; mono?: string}> {
  const loaded: {regular?: string; heading?: string; mono?: string} = {};
  const fontMap: Record<string, string> = {};

  if (fonts.regular) {
    const name = `theme-regular-${urlToHash(fonts.regular)}`;
    fontMap[name] = fonts.regular;
    loaded.regular = name;
  }
  if (fonts.heading) {
    const name = `theme-heading-${urlToHash(fonts.heading)}`;
    fontMap[name] = fonts.heading;
    loaded.heading = name;
  }
  if (fonts.mono) {
    const name = `theme-mono-${urlToHash(fonts.mono)}`;
    fontMap[name] = fonts.mono;
    loaded.mono = name;
  }

  if (Object.keys(fontMap).length === 0) return loaded;

  // Skip fonts that are already loaded
  const toLoad: Record<string, string> = {};
  for (const [name, source] of Object.entries(fontMap)) {
    if (!Font.isLoaded(name)) {
      toLoad[name] = source;
    }
  }

  if (Object.keys(toLoad).length > 0) {
    try {
      await Font.loadAsync(toLoad);
    } catch {
      // Font loading failure is non-fatal — return whatever was already loaded
      return {};
    }
  }

  return loaded;
}

export async function fetchRemoteTheme(url: string): Promise<ThemeConfig | null> {
  let config: ThemeConfig | null = null;

  try {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const json = await response.json();
    config = validateThemeConfig(json);
    if (config) {
      writeCachedTheme(url, config);
    }
  } catch {
    // Network failed, try cache
    config = await readCachedTheme(url);
  }

  if (!config) return null;

  // Load custom fonts if specified
  if (config.fonts) {
    const fontNames = await loadFonts(config.fonts);
    if (fontNames.regular || fontNames.heading || fontNames.mono) {
      const fontFamilies: Record<string, string> = {};
      if (fontNames.regular) fontFamilies.regular = fontNames.regular;
      if (fontNames.heading) fontFamilies.heading = fontNames.heading;
      if (fontNames.mono) fontFamilies.mono = fontNames.mono;

      config = {
        ...config,
        light: {
          ...config.light,
          fontFamilies: {...config.light.fontFamilies, ...fontFamilies},
        },
        dark: {
          ...config.dark,
          fontFamilies: {...config.dark.fontFamilies, ...fontFamilies},
        },
      };
    }
  }

  return config;
}
