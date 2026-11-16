import {describe, it, expect} from 'vitest';
import {getCachedFileMtime} from '../recentFiles';
import {__setMtime, __clearMtimes} from './mocks/expo-file-system-next';

describe('getCachedFileMtime', () => {
  it('returns the cache copy mtime for an entry id', () => {
    __clearMtimes();
    // Mock Paths.cache is '/mock-cache'; Directory/File join parts with '/'.
    __setMtime('/mock-cache/recent-files/abc123.md', 777);
    expect(getCachedFileMtime('abc123')).toBe(777);
  });

  it('returns null for a missing cache file', () => {
    __clearMtimes();
    expect(getCachedFileMtime('nope')).toBeNull();
  });
});
