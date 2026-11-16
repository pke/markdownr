import {describe, it, expect, beforeEach} from 'vitest';
import {resolveRelativeMarkdownLink} from '../useFileOpener';
import {Storage, StorageKeys} from '../settings';

const FILE = 'file:///docs/folder/a.md';

describe('resolveRelativeMarkdownLink', () => {
  beforeEach(() => {
    // No granted folder by default → containment falls back to the file's dir.
    Storage.setString(StorageKeys.LAST_FOLDER_URI, '');
  });

  it('returns null when there is no current file', () => {
    expect(resolveRelativeMarkdownLink('b.md', null)).toBeNull();
  });

  it('resolves a sibling relative link', () => {
    expect(resolveRelativeMarkdownLink('b.md', FILE)).toBe('file:///docs/folder/b.md');
  });

  it('resolves a subdirectory link', () => {
    expect(resolveRelativeMarkdownLink('sub/c.md', FILE)).toBe('file:///docs/folder/sub/c.md');
  });

  describe('rejects non-relative links', () => {
    it('anchors', () => {
      expect(resolveRelativeMarkdownLink('#section', FILE)).toBeNull();
    });

    it('protocol-relative //', () => {
      expect(resolveRelativeMarkdownLink('//evil.com/x.md', FILE)).toBeNull();
    });

    it.each([
      'http://evil.com/x',
      'https://evil.com/x',
      'mailto:a@b.com',
      'markdownr:home',
      'javascript:alert(1)',
      'file:///etc/passwd',
    ])('link carrying its own scheme: %s', (href) => {
      expect(resolveRelativeMarkdownLink(href, FILE)).toBeNull();
    });
  });

  describe('path traversal', () => {
    it('blocks ../ escaping the current directory when no folder is granted', () => {
      expect(resolveRelativeMarkdownLink('../secret.md', FILE)).toBeNull();
    });

    it('allows ../ that stays within the granted folder root', () => {
      Storage.setString(StorageKeys.LAST_FOLDER_URI, 'file:///docs/folder/');
      const cur = 'file:///docs/folder/sub/a.md';
      expect(resolveRelativeMarkdownLink('../b.md', cur)).toBe('file:///docs/folder/b.md');
    });

    it('blocks ../ that escapes the granted folder root', () => {
      Storage.setString(StorageKeys.LAST_FOLDER_URI, 'file:///docs/folder/');
      const cur = 'file:///docs/folder/sub/a.md';
      expect(resolveRelativeMarkdownLink('../../secret.md', cur)).toBeNull();
    });

    it('blocks deep traversal to the filesystem root', () => {
      expect(resolveRelativeMarkdownLink('../../../../../../etc/passwd', FILE)).toBeNull();
    });

    it('rejects percent-encoded dot-dot-slash (%2e%2e%2f)', () => {
      expect(resolveRelativeMarkdownLink('%2e%2e%2f%2e%2e%2fsecret.md', FILE)).toBeNull();
    });

    it('rejects an encoded slash (%2f) smuggling separators', () => {
      expect(resolveRelativeMarkdownLink('sub%2f..%2f..%2fsecret.md', FILE)).toBeNull();
    });
  });

  it('returns null when the current URI has no path separator', () => {
    expect(resolveRelativeMarkdownLink('b.md', 'noslash')).toBeNull();
  });
});
