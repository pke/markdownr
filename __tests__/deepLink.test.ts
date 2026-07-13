import {describe, it, expect} from 'vitest';
import {parseDeepLinkFileUri} from '../deepLink';

describe('parseDeepLinkFileUri', () => {
  it('returns null for empty input', () => {
    expect(parseDeepLinkFileUri(null, 'ios')).toBeNull();
    expect(parseDeepLinkFileUri(undefined, 'ios')).toBeNull();
    expect(parseDeepLinkFileUri('', 'ios')).toBeNull();
  });

  describe('iOS', () => {
    it('rebuilds a simple file:// URL', () => {
      expect(parseDeepLinkFileUri('file:///docs/note.md', 'ios')).toEqual({
        fileUri: 'file:///docs/note.md',
        name: 'note.md',
      });
    });

    it('decodes percent-encoded paths in the fileUri', () => {
      expect(parseDeepLinkFileUri('file:///docs/My%20Note.md', 'ios')?.fileUri)
        .toBe('file:///docs/My Note.md');
    });

    it('keeps the raw (still-encoded) last segment as the display name', () => {
      // name is derived from the raw URL, not the decoded path
      expect(parseDeepLinkFileUri('file:///docs/My%20Note.md', 'ios')?.name)
        .toBe('My%20Note.md');
    });

    it('returns null on malformed percent-encoding', () => {
      expect(parseDeepLinkFileUri('file:///docs/bad%.md', 'ios')).toBeNull();
    });
  });

  describe('Android', () => {
    it('uses a content:// URI as-is', () => {
      expect(parseDeepLinkFileUri('content://auth/document/42', 'android')).toEqual({
        fileUri: 'content://auth/document/42',
        name: '42',
      });
    });

    it('uses a file:// URI as-is (no decoding on Android)', () => {
      expect(parseDeepLinkFileUri('file:///storage/My%20Note.md', 'android')?.fileUri)
        .toBe('file:///storage/My%20Note.md');
    });
  });

  it('falls back to "Unknown" when there is no final path segment', () => {
    expect(parseDeepLinkFileUri('file:///docs/', 'ios')?.name).toBe('Unknown');
  });
});
