import {describe, it, expect, beforeEach, vi} from 'vitest';
import {_resetFileStore} from './mocks/expo-file-system-next';
import {_resetAllStores} from './mocks/react-native-mmkv';
import {addRecentFile, getRecentFiles, openRecentEntry} from '../recentFiles';
import type {FileSource} from '../fileChangeDetection';

const bookmark: FileSource = {uri: 'file:///private/var/Docs/a.md', kind: 'bookmark', ref: 'key-1'};

beforeEach(() => {
  _resetFileStore();
  _resetAllStores();
});

describe('openRecentEntry', () => {
  it('opens the live source when it is readable', async () => {
    addRecentFile('# Cached', 'a.md', bookmark);
    const openFile = vi.fn();
    const ok = await openRecentEntry(getRecentFiles()[0], openFile, async () => '# Live');
    expect(ok).toBe(true);
    expect(openFile).toHaveBeenCalledWith('# Live', 'a.md', bookmark.uri, bookmark);
  });

  it('falls back to the cache copy but keeps the source', async () => {
    addRecentFile('# Cached', 'a.md', bookmark);
    const openFile = vi.fn();
    const ok = await openRecentEntry(getRecentFiles()[0], openFile, async () => {
      throw new Error('not downloaded');
    });
    expect(ok).toBe(true);
    expect(openFile).toHaveBeenCalledWith('# Cached', 'a.md', bookmark.uri, bookmark);
  });

  it('opens entries without a source from the cache copy', async () => {
    addRecentFile('# Cached', 'a.md');
    const openFile = vi.fn();
    const readLive = vi.fn();
    expect(await openRecentEntry(getRecentFiles()[0], openFile, readLive)).toBe(true);
    expect(readLive).not.toHaveBeenCalled();
    expect(openFile).toHaveBeenCalledWith('# Cached', 'a.md', null, null);
  });

  it('returns false when neither the source nor the cache copy is readable', async () => {
    addRecentFile('# Cached', 'a.md', bookmark);
    const entry = getRecentFiles()[0];
    _resetFileStore();
    const openFile = vi.fn();
    expect(await openRecentEntry(entry, openFile, async () => { throw new Error('gone'); })).toBe(false);
    expect(openFile).not.toHaveBeenCalled();
  });
});
