import {describe, it, expect, vi} from 'vitest';
import {
  checkForExternalChange,
  shouldShowBanner,
  statFileSource,
  reloadFromSource,
  createChangeTracker,
  type FileSource,
} from '../fileChangeDetection';
import {__setMtime, __clearMtimes} from './mocks/expo-file-system-next';
import {
  __setPickedContent, __setPickedMtime, __resetFilePicker,
} from './mocks/file-picker';

const src = (uri = 'file:///docs/a.md'): FileSource => ({uri, kind: 'file'});
const statOf = (v: number | null) => vi.fn().mockResolvedValue(v);

describe('checkForExternalChange', () => {
  it('is unknown for null source or null baseline', async () => {
    expect((await checkForExternalChange(null, 100, statOf(200))).status).toBe('unknown');
    expect((await checkForExternalChange(src(), null, statOf(200))).status).toBe('unknown');
  });

  it('is changed when source mtime is newer than baseline', async () => {
    const check = await checkForExternalChange(src(), 100, statOf(200));
    expect(check).toEqual({status: 'changed', sourceMtime: 200});
  });

  it('is unchanged for equal or older mtimes', async () => {
    expect((await checkForExternalChange(src(), 200, statOf(200))).status).toBe('unchanged');
    expect((await checkForExternalChange(src(), 200, statOf(100))).status).toBe('unchanged');
  });

  it('is unknown when stat returns null, 0, or throws', async () => {
    expect((await checkForExternalChange(src(), 100, statOf(null))).status).toBe('unknown');
    expect((await checkForExternalChange(src(), 100, statOf(0))).status).toBe('unknown');
    const throwing = vi.fn().mockRejectedValue(new Error('gone'));
    expect((await checkForExternalChange(src(), 100, throwing)).status).toBe('unknown');
  });
});

describe('statFileSource (default stat strategy)', () => {
  it('reads modificationTime for file:// sources', async () => {
    __clearMtimes();
    __setMtime('file:///docs/a.md', 1234);
    expect(await statFileSource(src())).toBe(1234);
  });

  it('returns null for a file kind whose uri is not file://', async () => {
    expect(await statFileSource({uri: 'content://x/1', kind: 'file'})).toBeNull();
  });

  it('returns null when the file has no mtime (missing/unreadable)', async () => {
    __clearMtimes();
    expect(await statFileSource(src('file:///nope.md'))).toBeNull();
  });

  it('stats bookmark sources through the picker module ref', async () => {
    __resetFilePicker();
    __setPickedMtime('bm-key-1', 4321);
    expect(await statFileSource({uri: 'file:///icloud/x.md', kind: 'bookmark', ref: 'bm-key-1'})).toBe(4321);
  });

  it('stats content sources using the uri as the ref', async () => {
    __resetFilePicker();
    __setPickedMtime('content://docs/9', 5555);
    expect(await statFileSource({uri: 'content://docs/9', kind: 'content'})).toBe(5555);
  });

  it('returns null for a bookmark source without a ref', async () => {
    __resetFilePicker();
    expect(await statFileSource({uri: 'file:///x', kind: 'bookmark'})).toBeNull();
  });
});

describe('readFileSource (picked kinds)', () => {
  it('reads bookmark sources through the picker module', async () => {
    __resetFilePicker();
    __setPickedContent('bm-key-2', '# From iCloud');
    const {readFileSource} = await import('../fileChangeDetection');
    expect(await readFileSource({uri: 'file:///icloud/y.md', kind: 'bookmark', ref: 'bm-key-2'})).toBe('# From iCloud');
  });

  it('rejects for a bookmark source without a ref', async () => {
    __resetFilePicker();
    const {readFileSource} = await import('../fileChangeDetection');
    await expect(readFileSource({uri: 'file:///x', kind: 'bookmark'})).rejects.toThrow();
  });
});

describe('shouldShowBanner', () => {
  const changed = {status: 'changed' as const, sourceMtime: 300};

  it('shows for a changed status with no prior dismissal', () => {
    expect(shouldShowBanner(changed, null)).toBe(true);
  });

  it('never shows for unchanged or unknown', () => {
    expect(shouldShowBanner({status: 'unchanged', sourceMtime: 300}, null)).toBe(false);
    expect(shouldShowBanner({status: 'unknown', sourceMtime: null}, null)).toBe(false);
  });

  it('stays hidden for the same dismissed change, re-offers for a newer one', () => {
    expect(shouldShowBanner(changed, 300)).toBe(false); // same mtime dismissed
    expect(shouldShowBanner(changed, 400)).toBe(false); // dismissed even newer
    expect(shouldShowBanner({status: 'changed', sourceMtime: 500}, 300)).toBe(true);
  });
});

describe('reloadFromSource', () => {
  it('reads and re-opens, keeping the current file name', async () => {
    const read = vi.fn().mockResolvedValue('# New');
    const openFile = vi.fn();
    const ok = await reloadFromSource(src(), 'a.md', openFile, read);
    expect(ok).toBe(true);
    expect(read).toHaveBeenCalledWith(src());
    expect(openFile).toHaveBeenCalledWith('# New', 'a.md', 'file:///docs/a.md', src());
  });

  it('keeps a bookmark source intact so the reloaded file stays watchable', async () => {
    const bookmark: FileSource = {uri: 'file:///private/var/Docs/a.md', kind: 'bookmark', ref: 'key-1'};
    const read = vi.fn().mockResolvedValue('# New');
    const openFile = vi.fn();
    await reloadFromSource(bookmark, 'a.md', openFile, read);
    expect(openFile).toHaveBeenCalledWith('# New', 'a.md', bookmark.uri, bookmark);
  });

  it('returns false and does not open when the read fails', async () => {
    const read = vi.fn().mockRejectedValue(new Error('scope expired'));
    const openFile = vi.fn();
    expect(await reloadFromSource(src(), 'a.md', openFile, read)).toBe(false);
    expect(openFile).not.toHaveBeenCalled();
  });
});

describe('createChangeTracker', () => {
  const a = src('file:///docs/a.md');
  const b = src('file:///docs/b.md');
  const deferred = () => {
    let resolve!: (v: number | null) => void;
    const promise = new Promise<number | null>(r => { resolve = r; });
    return {promise, resolve};
  };
  const flush = () => new Promise(r => setTimeout(r, 0));

  it('uses the fresh stat as baseline and offers a newer edit once', async () => {
    const mtimes: Record<string, number> = {[a.uri]: 100};
    const tracker = createChangeTracker(async s => mtimes[s.uri] ?? null);
    tracker.track(a);
    await flush();
    expect(await tracker.check()).toBe(false);
    mtimes[a.uri] = 200;
    expect(await tracker.check()).toBe(true);
    tracker.dismiss();
    expect(await tracker.check()).toBe(false);
  });

  it('uses a given baseline without statting (launch restore)', async () => {
    const stat = vi.fn().mockResolvedValue(500);
    const tracker = createChangeTracker(stat);
    tracker.track(a, 300);
    expect(await tracker.check()).toBe(true);
    expect(stat).toHaveBeenCalledTimes(1);
  });

  it("drops a previous file's late baseline stat", async () => {
    const statA = deferred();
    const stat = vi.fn((s: FileSource) => (s.uri === a.uri ? statA.promise : Promise.resolve(100)));
    const tracker = createChangeTracker(stat);
    tracker.track(a);
    tracker.track(b);
    await flush(); // b's baseline = 100
    statA.resolve(50); // a's stat lands late; must not become b's baseline
    await flush();
    expect(await tracker.check()).toBe(false);
  });

  it('drops a focus check that finishes after another file was opened', async () => {
    const mtimes: Record<string, number> = {[a.uri]: 100, [b.uri]: 100};
    const pending = deferred();
    let hold = false;
    const tracker = createChangeTracker(s => (hold && s.uri === a.uri ? pending.promise : Promise.resolve(mtimes[s.uri])));
    tracker.track(a);
    await flush();
    hold = true;
    const check = tracker.check(); // in flight for a
    tracker.track(b); // "Open in…" delivers b
    pending.resolve(999); // a changed, but a is no longer displayed
    expect(await check).toBe(false);
    hold = false;
    await flush();
    expect(await tracker.check()).toBe(false);
  });
});
