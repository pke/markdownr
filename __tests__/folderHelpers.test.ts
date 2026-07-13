import {describe, it, expect} from 'vitest';
import {sortFiles, buildFolderIndex} from '../useFileOpener';

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
  it('builds a markdown index with relative links and stripped extensions', () => {
    const result = buildFolderIndex(
      [
        {name: '00-intro.md', uri: 'x', relativePath: '00-intro.md'},
        {name: 'a.md', uri: 'y', relativePath: 'sub/a.md'},
      ],
      'My Folder',
      'file:///base/first.md',
    );
    expect(result.uri).toBe('file:///base/first.md');
    expect(result.content).toContain('# My Folder');
    expect(result.content).toContain('- [00-intro](00-intro.md)');
    expect(result.content).toContain('- [a](sub/a.md)'); // label stripped, link uses relativePath
  });

  it('handles an empty folder', () => {
    const result = buildFolderIndex([], 'Empty', 'file:///base/');
    expect(result.content).toBe('# Empty\n');
  });
});
