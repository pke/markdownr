import {describe, it, expect, beforeEach} from 'vitest';
import {_resetFileStore} from './mocks/expo-file-system-next';
import {_resetAllStores} from './mocks/react-native-mmkv';
import {Storage, StorageKeys} from '../settings';
import {restoreSourceFor, type RecentFileEntry} from '../recentFiles';
import type {FileSource} from '../fileChangeDetection';

const entry = (subtitle: string, source?: FileSource): RecentFileEntry => ({
  id: '1', title: 'T', subtitle, addedAt: '2026-11-03T00:00:00Z', ...(source ? {source} : {}),
});
const persist = (source: FileSource) => Storage.setString(StorageKeys.LAST_FILE_SOURCE, JSON.stringify(source));

beforeEach(() => {
  _resetFileStore();
  _resetAllStores();
});

describe('restoreSourceFor', () => {
  it("prefers the entry's own source over the persisted last file", () => {
    const own: FileSource = {uri: 'file:///docs/b.md', kind: 'bookmark', ref: 'kb'};
    persist({uri: 'file:///docs/a.md', kind: 'bookmark', ref: 'ka'});
    expect(restoreSourceFor(entry('b.md', own))).toEqual(own);
  });

  it('ignores a persisted source that belongs to another file', () => {
    persist({uri: 'file:///docs/a.md', kind: 'file'});
    expect(restoreSourceFor(entry('b.md'))).toBeNull();
  });

  it('uses the persisted source for a legacy entry of the same file', () => {
    const last: FileSource = {uri: 'file:///docs/My%20Notes.md', kind: 'file'};
    persist(last);
    expect(restoreSourceFor(entry('My Notes.md'))).toEqual(last);
  });

  it('falls back to LAST_FILE_URI when no source was persisted', () => {
    Storage.setString(StorageKeys.LAST_FILE_URI, 'file:///docs/a.md');
    expect(restoreSourceFor(entry('a.md'))).toEqual({uri: 'file:///docs/a.md', kind: 'file'});
  });
});
