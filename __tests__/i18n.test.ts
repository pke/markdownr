import {describe, it, expect, afterEach} from 'vitest';
import {resources, currentLanguage} from '../i18n';
import {_setLanguageCode} from 'expo-localization';

// Flatten a nested translation object into dot-separated keys.
function flatten(obj: Record<string, unknown>, prefix = ''): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(obj)) {
    const full = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string') out[full] = value;
    else Object.assign(out, flatten(value as Record<string, unknown>, full));
  }
  return out;
}

describe('i18n translations', () => {
  const en = flatten(resources.en.translation);

  it.each(['de', 'ru'] as const)('%s has exactly the same keys as en', (lang) => {
    const t = flatten(resources[lang].translation);
    expect(Object.keys(t).sort()).toEqual(Object.keys(en).sort());
  });

  it.each(['en', 'de', 'ru'] as const)('%s has no empty strings', (lang) => {
    const t = flatten(resources[lang].translation);
    for (const [key, value] of Object.entries(t)) {
      expect(value.trim(), `${lang}.${key} is empty`).not.toBe('');
    }
  });
});

describe('currentLanguage', () => {
  afterEach(() => _setLanguageCode('en'));

  it('maps supported device locales to themselves', () => {
    _setLanguageCode('de');
    expect(currentLanguage()).toBe('de');
    _setLanguageCode('ru');
    expect(currentLanguage()).toBe('ru');
  });

  it('falls back to English for unsupported locales', () => {
    _setLanguageCode('fr');
    expect(currentLanguage()).toBe('en');
    _setLanguageCode('ja');
    expect(currentLanguage()).toBe('en');
  });
});
