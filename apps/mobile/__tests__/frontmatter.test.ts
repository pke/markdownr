import {describe, it, expect} from 'vitest';
import {parseFrontMatter, getExtraMetadata, hasExtraMetadata} from '../frontmatter';

describe('parseFrontMatter', () => {
  it('returns markdown unchanged when no front matter', () => {
    const input = '# Hello\n\nSome text';
    const result = parseFrontMatter(input);
    expect(result.frontMatter).toBeUndefined();
    expect(result.markdown).toBe(input);
  });

  it('parses basic front matter fields', () => {
    const input = `---
title: My Doc
author: John
date: 2025-01-15
---
# Content`;
    const result = parseFrontMatter(input);
    expect(result.frontMatter?.title).toBe('My Doc');
    expect(result.frontMatter?.author).toBe('John');
    expect(result.frontMatter?.date).toBe('2025-01-15');
    expect(result.markdown).toBe('# Content');
  });

  it('normalizes tags string to array', () => {
    const input = `---
tags: single-tag
---
Content`;
    const result = parseFrontMatter(input);
    expect(result.frontMatter?.tags).toEqual(['single-tag']);
  });

  it('keeps tags array as-is', () => {
    const input = `---
tags:
  - one
  - two
  - three
---
Content`;
    const result = parseFrontMatter(input);
    expect(result.frontMatter?.tags).toEqual(['one', 'two', 'three']);
  });

  it('normalizes Date objects to ISO date strings', () => {
    // js-yaml parses dates like "2025-01-15" as Date objects
    const input = `---
date: 2025-01-15
---
Content`;
    const result = parseFrontMatter(input);
    expect(typeof result.frontMatter?.date).toBe('string');
    expect(result.frontMatter?.date).toBe('2025-01-15');
  });

  it('handles invalid YAML gracefully', () => {
    const input = `---
: broken: yaml: [
---
Content`;
    const result = parseFrontMatter(input);
    // Should not throw, returns content after front matter
    expect(result.markdown).toBe('Content');
  });

  it('preserves extra fields', () => {
    const input = `---
title: Doc
category: tutorial
difficulty: hard
---
Content`;
    const result = parseFrontMatter(input);
    expect(result.frontMatter?.category).toBe('tutorial');
    expect(result.frontMatter?.difficulty).toBe('hard');
  });

  it('treats empty front matter as no front matter', () => {
    const input = `---
---
Content`;
    const result = parseFrontMatter(input);
    // Empty front matter (nothing between delimiters) doesn't match the regex
    expect(result.frontMatter).toBeUndefined();
  });
});

describe('getExtraMetadata', () => {
  it('returns empty object for standard-only fields', () => {
    const fm = {title: 'Test', author: 'Me', date: '2025-01-01', tags: ['a']};
    expect(getExtraMetadata(fm)).toEqual({});
  });

  it('returns extra fields excluding standard ones', () => {
    const fm = {title: 'Test', category: 'tutorial', version: 2};
    const extra = getExtraMetadata(fm);
    expect(extra).toEqual({category: 'tutorial', version: 2});
    expect(extra).not.toHaveProperty('title');
  });
});

describe('hasExtraMetadata', () => {
  it('returns false when only standard fields', () => {
    expect(hasExtraMetadata({title: 'Test'})).toBe(false);
  });

  it('returns true when extra fields present', () => {
    expect(hasExtraMetadata({title: 'Test', custom: 'value'})).toBe(true);
  });
});
