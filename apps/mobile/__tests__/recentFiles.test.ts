import {describe, it, expect, beforeEach} from 'vitest';
import {_resetFileStore, _getFileStore} from './mocks/expo-file-system-next';
import {_resetAllStores} from './mocks/react-native-mmkv';
import {
  getRecentFiles,
  addRecentFile,
  loadRecentFile,
  deleteRecentFile,
  clearAllRecentFiles,
} from '../recentFiles';

beforeEach(() => {
  _resetFileStore();
  _resetAllStores();
});

describe('getRecentFiles', () => {
  it('returns empty array when no files stored', () => {
    expect(getRecentFiles()).toEqual([]);
  });
});

describe('addRecentFile', () => {
  it('adds a file with filename', () => {
    addRecentFile('# Hello\nSome content', 'notes.md');
    const files = getRecentFiles();
    expect(files).toHaveLength(1);
    expect(files[0].title).toBe('Hello');
    expect(files[0].subtitle).toBe('notes.md');
  });

  it('derives title from front matter', () => {
    addRecentFile('---\ntitle: My Document\n---\n# Heading\nContent', 'doc.md');
    const files = getRecentFiles();
    expect(files[0].title).toBe('My Document');
    expect(files[0].subtitle).toBe('doc.md');
  });

  it('derives title from first heading when no front matter title', () => {
    addRecentFile('Some text\n## Second heading\n# First heading', 'readme.md');
    const files = getRecentFiles();
    // Should find the first heading in document order (## Second heading)
    expect(files[0].title).toBe('Second heading');
  });

  it('derives title from filename when no heading', () => {
    addRecentFile('Just plain text without headings', 'my-notes.md');
    const files = getRecentFiles();
    expect(files[0].title).toBe('my-notes');
  });

  it('uses ISO date for title when no filename and no heading', () => {
    addRecentFile('Just plain text', null);
    const files = getRecentFiles();
    // Title should be today's ISO date
    expect(files[0].title).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('uses ISO date as subtitle when no filename', () => {
    addRecentFile('# Some Title\nContent', null);
    const files = getRecentFiles();
    expect(files[0].title).toBe('Some Title');
    expect(files[0].subtitle).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('caches file content to disk', async () => {
    addRecentFile('# Test content', 'test.md');
    const files = getRecentFiles();
    const content = await loadRecentFile(files[0]);
    expect(content).toBe('# Test content');
  });

  it('prepends new entries (most recent first)', () => {
    addRecentFile('# First', 'first.md');
    addRecentFile('# Second', 'second.md');
    const files = getRecentFiles();
    expect(files).toHaveLength(2);
    expect(files[0].subtitle).toBe('second.md');
    expect(files[1].subtitle).toBe('first.md');
  });

  it('deduplicates by subtitle and moves to front', () => {
    addRecentFile('# Version 1', 'notes.md');
    addRecentFile('# Other', 'other.md');
    addRecentFile('# Version 2', 'notes.md');
    const files = getRecentFiles();
    expect(files).toHaveLength(2);
    expect(files[0].subtitle).toBe('notes.md');
    expect(files[0].title).toBe('Version 2');
    expect(files[1].subtitle).toBe('other.md');
  });

  it('enforces max 10 entries', () => {
    for (let i = 0; i < 12; i++) {
      addRecentFile(`# File ${i}`, `file-${i}.md`);
    }
    const files = getRecentFiles();
    expect(files).toHaveLength(10);
    // Most recent should be first
    expect(files[0].subtitle).toBe('file-11.md');
    // Oldest two should be evicted
    expect(files.find(f => f.subtitle === 'file-0.md')).toBeUndefined();
    expect(files.find(f => f.subtitle === 'file-1.md')).toBeUndefined();
  });

  it('cleans up evicted files from disk', async () => {
    for (let i = 0; i < 12; i++) {
      addRecentFile(`# File ${i}`, `file-${i}.md`);
    }
    // The evicted files' content should no longer be on disk
    const store = _getFileStore();
    // Should have exactly 10 files
    const mdFiles = [...store.keys()].filter(k => k.includes('recent-files'));
    expect(mdFiles).toHaveLength(10);
  });
});

describe('deleteRecentFile', () => {
  it('removes entry from list and disk', async () => {
    addRecentFile('# Test', 'test.md');
    const files = getRecentFiles();
    expect(files).toHaveLength(1);

    deleteRecentFile(files[0].id);
    expect(getRecentFiles()).toHaveLength(0);

    const content = await loadRecentFile(files[0]);
    expect(content).toBeNull();  // File deleted, not found on disk
  });
});

describe('clearAllRecentFiles', () => {
  it('removes all entries and files', () => {
    addRecentFile('# One', 'one.md');
    addRecentFile('# Two', 'two.md');
    addRecentFile('# Three', 'three.md');
    expect(getRecentFiles()).toHaveLength(3);

    clearAllRecentFiles();
    expect(getRecentFiles()).toHaveLength(0);
  });
});

describe('loadRecentFile', () => {
  it('returns null for missing file', async () => {
    const result = await loadRecentFile({id: 'nonexistent', title: 'X', subtitle: 'x.md', addedAt: ''});
    expect(result).toBeNull();
  });

  it('returns cached content', async () => {
    addRecentFile('# My Content\nWith paragraphs', 'doc.md');
    const files = getRecentFiles();
    const content = await loadRecentFile(files[0]);
    expect(content).toBe('# My Content\nWith paragraphs');
  });
});

describe('loadRecentFile after cache purge', () => {
  it('returns null when iOS purged the cached file', async () => {
    addRecentFile('# Cached', 'cached.md');
    const files = getRecentFiles();

    // Simulate iOS purging the cached file
    const store = _getFileStore();
    const key = [...store.keys()].find(k => k.includes(files[0].id))!;
    store.delete(key);

    const content = await loadRecentFile(files[0]);
    expect(content).toBeNull();
    // Entry still exists in metadata until user interacts
    expect(getRecentFiles()).toHaveLength(1);
  });
});

describe('title derivation edge cases', () => {
  it('handles h6 heading', () => {
    addRecentFile('###### Small heading\nContent', 'test.md');
    expect(getRecentFiles()[0].title).toBe('Small heading');
  });

  it('prefers front matter title over heading', () => {
    addRecentFile('---\ntitle: FM Title\n---\n# Heading Title', 'test.md');
    expect(getRecentFiles()[0].title).toBe('FM Title');
  });

  it('prefers heading over filename', () => {
    addRecentFile('No front matter\n# Doc Heading', 'my-file.md');
    expect(getRecentFiles()[0].title).toBe('Doc Heading');
  });

  it('strips .markdown extension from filename fallback', () => {
    addRecentFile('No headings here', 'readme.markdown');
    expect(getRecentFiles()[0].title).toBe('readme');
  });
});
