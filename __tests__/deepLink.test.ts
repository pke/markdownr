import {describe, it, expect, vi} from 'vitest';
import {parseDeepLinkFileUri, openDeepLink} from '../deepLink';

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

describe('openDeepLink', () => {
  it('reads the file and opens it with the parsed name + uri', async () => {
    const readFileText = vi.fn().mockResolvedValue('# Hello\n\nbody');
    const openFile = vi.fn();

    const opened = await openDeepLink('file:///docs/note.md', readFileText, openFile, 'ios');

    expect(opened).toBe(true);
    expect(readFileText).toHaveBeenCalledWith('file:///docs/note.md');
    expect(openFile).toHaveBeenCalledWith('# Hello\n\nbody', 'note.md', 'file:///docs/note.md');
  });

  it('reads a content:// uri as-is on Android', async () => {
    const readFileText = vi.fn().mockResolvedValue('body');
    const openFile = vi.fn();

    await openDeepLink('content://auth/document/42', readFileText, openFile, 'android');

    expect(readFileText).toHaveBeenCalledWith('content://auth/document/42');
    expect(openFile).toHaveBeenCalledWith('body', '42', 'content://auth/document/42');
  });

  it('does nothing and returns false for a non-file link', async () => {
    const readFileText = vi.fn();
    const openFile = vi.fn();

    const opened = await openDeepLink(null, readFileText, openFile, 'ios');

    expect(opened).toBe(false);
    expect(readFileText).not.toHaveBeenCalled();
    expect(openFile).not.toHaveBeenCalled();
  });

  it('propagates read errors without opening', async () => {
    const readFileText = vi.fn().mockRejectedValue(new Error('purged'));
    const openFile = vi.fn();

    await expect(openDeepLink('file:///docs/gone.md', readFileText, openFile, 'ios')).rejects.toThrow('purged');
    expect(openFile).not.toHaveBeenCalled();
  });
});
