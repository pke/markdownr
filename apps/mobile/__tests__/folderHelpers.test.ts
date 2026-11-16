import {describe, it, expect, vi} from 'vitest';
import {openFolderResult} from '../useFileOpener';
import {sortFiles, buildFolderIndex, resolveFolderLink, updateFolderLocation, folderIndex} from '../folderNavigation';

const f = (name: string, relativePath = name) => ({name, uri: `file:///${relativePath}`, relativePath});

describe('sortFiles', () => {
  it('orders by numeric prefix', () => {
    const sorted = sortFiles([f('02-c.md'), f('00-a.md'), f('01-b.md')]);
    expect(sorted.map(x => x.name)).toEqual(['00-a.md', '01-b.md', '02-c.md']);
  });

  it('compares numeric prefixes numerically, not lexically', () => {
    const sorted = sortFiles([f('10-x.md'), f('2-y.md')]);
    expect(sorted.map(x => x.name)).toEqual(['2-y.md', '10-x.md']);
  });

  it('places numeric-prefixed files before non-numeric ones', () => {
    const sorted = sortFiles([f('readme.md'), f('01-intro.md')]);
    expect(sorted.map(x => x.name)).toEqual(['01-intro.md', 'readme.md']);
  });

  it('sorts non-numeric files by relative path', () => {
    const sorted = sortFiles([f('zebra.md', 'z/zebra.md'), f('apple.md', 'a/apple.md')]);
    expect(sorted.map(x => x.relativePath)).toEqual(['a/apple.md', 'z/zebra.md']);
  });

  it('does not mutate the input array', () => {
    const input = [f('b.md'), f('a.md')];
    const copy = [...input];
    sortFiles(input);
    expect(input).toEqual(copy);
  });
});

describe('buildFolderIndex', () => {
  it('encodes spaces, parentheses and fragment characters as part of the filename', () => {
    const result = buildFolderIndex([f('Plan (v2)#.md', 'Notes here/Plan (v2)#.md')], 'Notes');
    expect(result).toContain('[Plan (v2)#](Notes%20here/Plan%20%28v2%29%23.md)');
  });
  it('builds a markdown index with relative links and stripped extensions', () => {
    const result = buildFolderIndex(
      [
        {name: '00-intro.md', uri: 'x', relativePath: '00-intro.md'},
        {name: 'a.md', uri: 'y', relativePath: 'sub/a.md'},
      ],
      'My Folder',
    );
    expect(result).toContain('# My Folder');
    expect(result).toContain('- [00-intro](00-intro.md)');
    expect(result).toContain('- [a](sub/a.md)');
  });

  it('handles an empty folder', () => {
    expect(buildFolderIndex([], 'Empty')).toBe('# Empty\n');
  });
});

describe('folder link navigation', () => {
  const file = {name: 'Plan (v2)#.md', relativePath: 'Notes here/Plan (v2)#.md', uri: 'content://provider/tree/42/document/867'};
  const result = {folderName: 'Manual', folderUri: 'content://provider/tree/42', files: [file]};

  it('opens a link in the index using its enumerated SAF URI', () => {
    expect(resolveFolderLink('Notes%20here/Plan%20%28v2%29%23.md', {result})).toEqual(file);
  });

  it('resolves a relative link from the current document path', () => {
    expect(resolveFolderLink('../Notes%20here/Plan%20%28v2%29%23.md#section', {
      result, relativePath: 'intro/start.md',
    })).toEqual(file);
  });

  it.each(['../Notes%20here/Plan%20%28v2%29%23.md', 'https://elsewhere/Notes%20here/Plan%20%28v2%29%23.md', 'Notes%20here%2FPlan%20%28v2%29%23.md'])(
    'does not resolve %s from the folder index', href => {
      expect(resolveFolderLink(href, {result})).toBeNull();
    },
  );
});

describe('openFolderResult', () => {
  it('opens the generated index with its own folder location and no source to watch', async () => {
    const openFile = vi.fn();
    const result = {folderUri: 'file:///folder', folderName: 'Notes', files: [f('b.md'), f('a.md')]};
    await openFolderResult(result, openFile);
    expect(openFile).toHaveBeenCalledTimes(1);
    const [content, name, uri, source, folder] = openFile.mock.calls[0];
    expect(content).toContain('# Notes');
    expect(name).toBe('Notes');
    expect(uri).toBeNull();
    expect(source).toBeNull();
    expect(folder).toEqual({result});
  });
});

describe('refreshing the active folder', () => {
  const original = {folderUri: 'file:///external', folderName: 'Handbook', bookmarkKey: 'handbook', files: [f('Old.md'), f('Removed.md')]};
  const updated = {...original, files: [f('Renamed.md'), f('Added.md')]};

  it('regenerates the index using the new set of files', () => {
    const location = updateFolderLocation({result: original}, updated);
    expect(location && folderIndex(location.result)[0]).toBe('# Handbook\n\n- [Added](Added.md)\n- [Renamed](Renamed.md)');
  });

  it('keeps the current file path when refreshing its folder', () => {
    const location = updateFolderLocation({result: original, relativePath: 'Old.md'}, updated);
    expect(location).toEqual({result: updated, relativePath: 'Old.md'});
  });

  it('discards a refresh that belongs to another folder', () => {
    expect(updateFolderLocation({result: original}, {...updated, bookmarkKey: 'other'})).toBeNull();
  });
});
