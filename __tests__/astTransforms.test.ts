import {describe, it, expect} from 'vitest';
import {htmlInlineTransform, quoteCycleTransform, preprocessMarkdownHtml} from '../astTransforms';

// Minimal MarkdownNode helper
function doc(...children: object[]) {
  return {type: 'document', children};
}
function para(...children: object[]) {
  return {type: 'paragraph', children};
}
function text(content: string) {
  return {type: 'text', content};
}
function htmlInline(content: string) {
  return {type: 'html_inline', content};
}
function blockquote(...children: object[]) {
  return {type: 'blockquote', children};
}

// ---------------------------------------------------------------------------
// htmlInlineTransform
// ---------------------------------------------------------------------------

describe('htmlInlineTransform', () => {
  it('replaces <br> with line_break', () => {
    const ast = doc(para(text('hello'), htmlInline('<br>'), text('world')));
    const result = htmlInlineTransform(ast as any);
    expect(result.children![0].children![1]).toEqual({type: 'line_break'});
  });

  it('replaces self-closing <br/>', () => {
    const ast = doc(para(htmlInline('<br/>')));
    const result = htmlInlineTransform(ast as any);
    expect(result.children![0].children![0]).toEqual({type: 'line_break'});
  });

  it('replaces <br /> with space before slash', () => {
    const ast = doc(para(htmlInline('<br />')));
    const result = htmlInlineTransform(ast as any);
    expect(result.children![0].children![0]).toEqual({type: 'line_break'});
  });

  it('is case-insensitive', () => {
    const ast = doc(para(htmlInline('<BR>'), htmlInline('<BR/>')));
    const result = htmlInlineTransform(ast as any);
    expect(result.children![0].children![0]).toEqual({type: 'line_break'});
    expect(result.children![0].children![1]).toEqual({type: 'line_break'});
  });

  it('leaves other html_inline tags unchanged', () => {
    const node = htmlInline('<span>');
    const ast = doc(para(node));
    const result = htmlInlineTransform(ast as any);
    expect(result.children![0].children![0]).toEqual({type: 'html_inline', content: '<span>'});
  });

  it('leaves non-html nodes unchanged', () => {
    const ast = doc(para(text('hello'), text('world')));
    const result = htmlInlineTransform(ast as any);
    expect(result.children![0].children![0]).toEqual({type: 'text', content: 'hello'});
    expect(result.children![0].children![1]).toEqual({type: 'text', content: 'world'});
  });

  it('handles nested nodes recursively', () => {
    const ast = doc(
      {type: 'blockquote', children: [para(htmlInline('<br>'), text('text'))]},
    );
    const result = htmlInlineTransform(ast as any);
    expect(result.children![0].children![0].children![0]).toEqual({type: 'line_break'});
  });
});

// ---------------------------------------------------------------------------
// quoteCycleTransform
// ---------------------------------------------------------------------------

function quotesBlock(...quotes: object[]) {
  return [
    para(text(':::quotes')),
    ...quotes,
    para(text(':::')),
  ];
}

describe('quoteCycleTransform', () => {
  it('replaces :::quotes block with a quote_cycle node containing all blockquotes', () => {
    const q0 = blockquote(para(text('Quote 0')));
    const q1 = blockquote(para(text('Quote 1')));
    const ast = doc(...quotesBlock(q0, q1));

    const result = quoteCycleTransform(ast as any);
    expect(result.children).toHaveLength(1);
    expect(result.children![0].type).toBe('quote_cycle');
    expect(result.children![0].children).toHaveLength(2);
  });

  it('collects all blockquotes as children of quote_cycle', () => {
    const q0 = blockquote(para(text('Quote 0')));
    const q1 = blockquote(para(text('Quote 1')));
    const q2 = blockquote(para(text('Quote 2')));
    const ast = doc(...quotesBlock(q0, q1, q2));

    const result = quoteCycleTransform(ast as any);
    expect(result.children![0].children).toHaveLength(3);
    // All children should be blockquotes (possibly shuffled)
    for (const child of result.children![0].children!) {
      expect(child.type).toBe('blockquote');
    }
  });

  it('leaves document unchanged when no :::quotes marker', () => {
    const ast = doc(para(text('No quotes here')));
    const result = quoteCycleTransform(ast as any);
    expect(result.children).toHaveLength(1);
    expect(result.children![0]).toEqual(para(text('No quotes here')));
  });

  it('preserves surrounding content', () => {
    const q0 = blockquote(para(text('A quote')));
    const ast = doc(
      para(text('Before')),
      ...quotesBlock(q0),
      para(text('After')),
    );
    const result = quoteCycleTransform(ast as any);
    expect(result.children).toHaveLength(3);
    expect(result.children![0]).toEqual(para(text('Before')));
    expect(result.children![1].type).toBe('quote_cycle');
    expect(result.children![2]).toEqual(para(text('After')));
  });

  it('returns unchanged ast when :::quotes has no closing :::', () => {
    const ast = doc(para(text(':::quotes')), blockquote(para(text('A quote'))));
    const result = quoteCycleTransform(ast as any);
    // No closing ::: → no transformation
    expect(result.children![0]).toEqual(para(text(':::quotes')));
  });
});

// ---------------------------------------------------------------------------
// preprocessMarkdownHtml
// ---------------------------------------------------------------------------

describe('preprocessMarkdownHtml', () => {
  it('converts <br> to hard line break', () => {
    expect(preprocessMarkdownHtml('line1<br>line2')).toBe('line1  \nline2');
  });

  it('converts self-closing <br/> and <br /> variants', () => {
    expect(preprocessMarkdownHtml('a<br/>b')).toBe('a  \nb');
    expect(preprocessMarkdownHtml('a<br />b')).toBe('a  \nb');
  });

  it('is case-insensitive for <BR>', () => {
    expect(preprocessMarkdownHtml('a<BR>b')).toBe('a  \nb');
  });

  it('converts <b>...</b> to **...**', () => {
    expect(preprocessMarkdownHtml('<b>bold</b>')).toBe('**bold**');
  });

  it('converts <strong>...</strong> to **...**', () => {
    expect(preprocessMarkdownHtml('<strong>text</strong>')).toBe('**text**');
  });

  it('converts <i>...</i> to *...*', () => {
    expect(preprocessMarkdownHtml('<i>italic</i>')).toBe('*italic*');
  });

  it('converts <em>...</em> to *...*', () => {
    expect(preprocessMarkdownHtml('<em>text</em>')).toBe('*text*');
  });

  it('converts <s>, <del>, <strike> to ~~...~~', () => {
    expect(preprocessMarkdownHtml('<s>gone</s>')).toBe('~~gone~~');
    expect(preprocessMarkdownHtml('<del>old</del>')).toBe('~~old~~');
    expect(preprocessMarkdownHtml('<strike>x</strike>')).toBe('~~x~~');
  });

  it('converts <u> and <ins> to ++...++', () => {
    expect(preprocessMarkdownHtml('<u>under</u>')).toBe('++under++');
    expect(preprocessMarkdownHtml('<ins>added</ins>')).toBe('++added++');
  });

  it('converts <mark> to ==...==', () => {
    expect(preprocessMarkdownHtml('<mark>hi</mark>')).toBe('==hi==');
  });

  it('converts <sup> and <sub>', () => {
    expect(preprocessMarkdownHtml('x<sup>2</sup>')).toBe('x^2^');
    expect(preprocessMarkdownHtml('H<sub>2</sub>O')).toBe('H~2~O');
  });

  it('converts <code> to backtick span', () => {
    expect(preprocessMarkdownHtml('<code>fn()</code>')).toBe('`fn()`');
  });

  it('does not replace tags inside backtick code spans', () => {
    expect(preprocessMarkdownHtml('text `<br>` more')).toBe('text `<br>` more');
    expect(preprocessMarkdownHtml('text `<b>bold</b>` more')).toBe('text `<b>bold</b>` more');
  });

  it('does not replace tags inside fenced code blocks', () => {
    const input = '```\n<b>bold</b>\n<br>\n```';
    expect(preprocessMarkdownHtml(input)).toBe(input);
  });

  it('replaces tags outside code while preserving code', () => {
    const input = '<b>before</b> `<b>code</b>` <i>after</i>';
    expect(preprocessMarkdownHtml(input)).toBe('**before** `<b>code</b>` *after*');
  });
});
