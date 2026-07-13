import {describe, it, expect, beforeEach, afterEach, vi} from 'vitest';
import {resolveThemeFromFrontMatter, validateThemeConfig, fetchRemoteTheme} from '../themeLoader';
import {_resetFileStore} from './mocks/expo-file-system-next';

const validColors = {
  text: '#000', background: '#fff', heading: '#000',
  link: '#00f', border: '#ccc', codeBackground: '#eee',
};
const validTheme = {background: {light: '#fff', dark: '#000'}, light: {colors: validColors}};

describe('resolveThemeFromFrontMatter', () => {
  it('resolves a built-in theme name', () => {
    const r = resolveThemeFromFrontMatter('ocean');
    expect(r?.type).toBe('builtin');
    expect(r && r.type === 'builtin' && r.name).toBe('ocean');
  });

  it('resolves http(s) URLs as remote', () => {
    expect(resolveThemeFromFrontMatter('https://x.com/t.json')).toEqual({type: 'remote', url: 'https://x.com/t.json'});
    expect(resolveThemeFromFrontMatter('http://x.com/t.json')).toEqual({type: 'remote', url: 'http://x.com/t.json'});
  });

  it('returns null for unknown names and non-http URLs', () => {
    expect(resolveThemeFromFrontMatter('notatheme')).toBeNull();
    expect(resolveThemeFromFrontMatter('ftp://x.com/t.json')).toBeNull();
  });
});

describe('validateThemeConfig', () => {
  it('rejects non-objects', () => {
    expect(validateThemeConfig(null)).toBeNull();
    expect(validateThemeConfig('x')).toBeNull();
    expect(validateThemeConfig([])).toBeNull();
  });

  it('rejects a missing or incomplete background', () => {
    expect(validateThemeConfig({light: {colors: validColors}})).toBeNull();
    expect(validateThemeConfig({background: {light: '#fff'}, light: {colors: validColors}})).toBeNull();
  });

  it('rejects configs without required colors on either scheme', () => {
    expect(validateThemeConfig({background: {light: '#fff', dark: '#000'}})).toBeNull();
    expect(validateThemeConfig({
      background: {light: '#fff', dark: '#000'},
      light: {colors: {text: '#000'}}, // incomplete
    })).toBeNull();
  });

  it('accepts a valid config and fills the default icon', () => {
    const r = validateThemeConfig(validTheme);
    expect(r).not.toBeNull();
    expect(r?.icon).toBe('🎨');
    expect(r?.background).toEqual({light: '#fff', dark: '#000'});
  });

  it('keeps a provided icon and author', () => {
    const r = validateThemeConfig({...validTheme, icon: '🌊', author: 'me'});
    expect(r?.icon).toBe('🌊');
    expect(r?.author).toBe('me');
  });
});

describe('fetchRemoteTheme', () => {
  beforeEach(() => _resetFileStore());
  afterEach(() => vi.unstubAllGlobals());

  it('fetches and validates a remote theme', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ok: true, json: async () => validTheme}));
    const r = await fetchRemoteTheme('https://x.com/t.json');
    expect(r).not.toBeNull();
    expect(r?.background).toEqual({light: '#fff', dark: '#000'});
  });

  it('returns null on a non-ok response with no cache', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ok: false, status: 404}));
    expect(await fetchRemoteTheme('https://x.com/missing.json')).toBeNull();
  });

  it('returns null when fetch throws and there is no cache', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('network')));
    expect(await fetchRemoteTheme('https://x.com/err.json')).toBeNull();
  });

  it('falls back to a cached theme when the network fails', async () => {
    // First, a successful fetch populates the cache.
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ok: true, json: async () => validTheme}));
    await fetchRemoteTheme('https://x.com/cached.json');
    // Then the network fails but the cache serves it.
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
    const r = await fetchRemoteTheme('https://x.com/cached.json');
    expect(r).not.toBeNull();
    expect(r?.background).toEqual({light: '#fff', dark: '#000'});
  });
});
