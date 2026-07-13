let _languageCode = 'en';

export function getLocales() {
  return [{languageCode: _languageCode}];
}

// Test helper: override the reported device language.
export function _setLanguageCode(code: string): void {
  _languageCode = code;
}
