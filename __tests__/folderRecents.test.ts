import {beforeEach, describe, expect, it, vi} from 'vitest';
import {addRecentFile, getRecentFiles, openRecentEntry, restoreRecentDocument} from '../recentFiles';
import {folderContents, folderMtimes, restoredFolders} from './mocks/folder-picker';
import {__setPickedContent, __resetFilePicker} from './mocks/file-picker';
import {_resetAllStores} from './mocks/react-native-mmkv';
import {_resetFileStore} from './mocks/expo-file-system-next';
import {createChangeTracker, readFileSource} from '../fileChangeDetection';

const intro = {name: '00-intro.md', uri: 'content://provider/tree/manual/document/23', relativePath: '00-intro.md'};
const chapter = {name: 'details.md', uri: 'content://provider/tree/manual/document/47', relativePath: 'sub/details.md'};
const manual = {folderUri: 'content://provider/tree/manual', folderName: 'Manual', files: [intro, chapter]};
const introSource = {kind: 'content', uri: intro.uri} as const;
const chapterSource = {kind: 'content', uri: chapter.uri} as const;

beforeEach(() => {
  _resetAllStores();
  _resetFileStore();
  __resetFilePicker();
  restoredFolders.clear();
  folderContents.clear();
  folderMtimes.clear();
  restoredFolders.set(manual.folderUri, manual);
  __setPickedContent(chapter.uri, '# Live chapter');
});

function readChapter() {
  addRecentFile('# Saved chapter', chapter.name, chapterSource, {result: manual, relativePath: chapter.relativePath});
}

describe('folder history', () => {
  it('keeps the latest file under a single entry for the folder', () => {
    addRecentFile('# Intro', intro.name, introSource, {result: manual, relativePath: intro.relativePath});
    readChapter();
    expect(getRecentFiles()).toHaveLength(1);
    expect(getRecentFiles()[0]).toMatchObject({title: 'Manual', subtitle: 'details.md', folderPath: 'sub/details.md'});
  });

  it('keeps similarly named folders and separately opened documents accessible', () => {
    readChapter();
    addRecentFile('# Another', chapter.name, {kind: 'content', uri: 'content://provider/elsewhere'}, {
      result: {...manual, folderUri: 'content://provider/tree/another'}, relativePath: chapter.relativePath,
    });
    addRecentFile('# Standalone', chapter.name, {kind: 'file', uri: 'file:///details.md'});
    expect(getRecentFiles().map(entry => entry.title)).toEqual(['Standalone', 'Manual', 'Manual']);
  });

  it('opens the refreshed index via the folder target and keeps its latest file', async () => {
    readChapter();
    restoredFolders.set(manual.folderUri, {...manual, files: [intro, chapter, {name: 'New.md', uri: 'content://provider/document/88', relativePath: 'New.md'}]});
    const open = vi.fn();
    expect(await openRecentEntry(getRecentFiles()[0], open, {target: 'folder'})).toBe(true);
    expect(open.mock.calls[0][0]).toContain('[New](New.md)');
    expect(open.mock.calls[0][4].relativePath).toBeUndefined();
    const [content, name, , , location] = open.mock.calls[0];
    addRecentFile(content, name, undefined, location);
    expect(getRecentFiles()[0].subtitle).toBe('details.md');
  });

  it('opens the last file directly via the file target', async () => {
    readChapter();
    const open = vi.fn();
    expect(await openRecentEntry(getRecentFiles()[0], open)).toBe(true);
    expect(open.mock.calls[0].slice(0, 3)).toEqual(['# Live chapter', 'details.md', chapter.uri]);
  });
});

describe('launch restore', () => {
  it('restores the last folder file even when a numeric first file exists', async () => {
    readChapter();
    const document = await restoreRecentDocument(getRecentFiles()[0]);
    expect(document?.slice(0, 3)).toEqual(['# Live chapter', 'details.md', chapter.uri]);
  });

  it('restores the fresh folder index if the last file disappeared', async () => {
    readChapter();
    restoredFolders.set(manual.folderUri, {...manual, files: [intro]});
    const document = await restoreRecentDocument(getRecentFiles()[0]);
    expect(document?.[0]).toContain('[00-intro](00-intro.md)');
    expect(document?.[4]?.relativePath).toBeUndefined();
  });

  it('recognizes the previous Android SAF folder from its known document URI', async () => {
    addRecentFile('# Old standalone format', chapter.name, chapterSource);
    const document = await restoreRecentDocument(getRecentFiles()[0], manual.folderUri);
    expect(document?.[4]?.relativePath).toBe('sub/details.md');
  });
});

it('reads and checks the original iOS folder document through its bookmark', async () => {
  const source = {kind: 'folder', ref: 'manual-bookmark', relativePath: 'sub/details.md', uri: 'file:///external/sub/details.md'} as const;
  folderContents.set('manual-bookmark/sub/details.md', '# Original chapter');
  folderMtimes.set('manual-bookmark/sub/details.md', 200);
  expect(await readFileSource(source)).toBe('# Original chapter');
  const tracker = createChangeTracker();
  tracker.track(source, 100);
  expect(await tracker.check()).toBe(true);
});
