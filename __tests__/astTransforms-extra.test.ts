import {describe, it, expect} from 'vitest';
import {
  composeTransforms,
  createTypographicTransform,
  emoticonTransform,
  subSuperscriptTransform,
  insMarkTransform,
  definitionListTransform,
  abbreviationTransform,
  footnoteTransform,
  tableBrTransform,
  TABLE_BR_PLACEHOLDER,
} from '../astTransforms';
import {emojiData} from '../emojiData';

// --- minimal AST helpers ---------------------------------------------------
const doc = (...children: any[]) => ({type: 'document', children});
const para = (...children: any[]) => ({type: 'paragraph', children});
const text = (content: string) => ({type: 'text', content});
const getContent = (node: any): string =>
  node.content ?? (node.children ?? []).map(getContent).join('');

// ---------------------------------------------------------------------------
describe('composeTransforms', () => {
  it('applies transforms left to right', () => {
    const order: string[] = [];
    const a = (ast: any) => { order.push('a'); ast.a = true; return ast; };
    const b = (ast: any) => { order.push('b'); ast.sawA = ast.a === true; return ast; };
    const result: any = composeTransforms(a as any, b as any)(doc() as any);
    expect(order).toEqual(['a', 'b']);
    expect(result.sawA).toBe(true);
  });

  it('returns the ast unchanged with no transforms', () => {
    const ast = doc(para(text('x')));
    expect(composeTransforms()(ast as any)).toBe(ast);
  });
});

// ---------------------------------------------------------------------------
describe('createTypographicTransform', () => {
  const apply = (locale: string, content: string) => {
    const ast = doc(para(text(content)));
    createTypographicTransform(locale)(ast as any);
    return (ast as any).children[0].children[0].content;
  };

  it('replaces (c) (r) (tm) symbols', () => {
    expect(apply('en', '(c) (r) (tm)')).toBe('© ® ™');
  });

  it('converts -- to en-dash and --- to em-dash', () => {
    expect(apply('en', 'a --- b')).toContain('—');
    expect(apply('en', 'a -- b')).toContain('–');
  });

  it('converts ... to an ellipsis', () => {
    expect(apply('en', 'wait...')).toBe('wait…');
  });

  it('uses English smart double quotes', () => {
    expect(apply('en', '"hi"')).toBe('“hi”');
  });

  it('uses German smart double quotes', () => {
    expect(apply('de', '"hi"')).toBe('„hi“');
  });

  it('converts +- to plus-minus', () => {
    expect(apply('en', '5 +- 1')).toBe('5 ± 1');
  });

  it('leaves text inside code spans untouched', () => {
    const inner = text('(c)');
    const ast = doc(para({type: 'code_inline', children: [inner]}));
    createTypographicTransform('en')(ast as any);
    expect(inner.content).toBe('(c)');
  });
});

// ---------------------------------------------------------------------------
describe('emoticonTransform', () => {
  const apply = (content: string) => {
    const ast = doc(para(text(content)));
    emoticonTransform(ast as any);
    return (ast as any).children[0].children[0].content;
  };

  it('replaces a :shortcode: with its emoji', () => {
    expect(apply(':smile:')).toBe(emojiData.get('smile'));
  });

  it('replaces the :) emoticon with the smiley emoji', () => {
    expect(apply(':)')).toBe(emojiData.get('smiley'));
  });

  it('leaves unknown shortcodes unchanged', () => {
    expect(apply(':definitelynotacode:')).toBe(':definitelynotacode:');
  });
});

// ---------------------------------------------------------------------------
describe('subSuperscriptTransform', () => {
  const apply = (content: string) => {
    const ast = doc(para(text(content)));
    subSuperscriptTransform(ast as any);
    return (ast as any).children[0].children[0].content;
  };

  it('superscripts ^...^', () => {
    expect(apply('x^2^')).toBe('x²');
  });

  it('subscripts ~...~', () => {
    expect(apply('H~2~O')).toBe('H₂O');
  });

  it('handles letters', () => {
    expect(apply('19^th^')).toBe('19ᵗʰ');
  });
});

// ---------------------------------------------------------------------------
describe('insMarkTransform', () => {
  it('wraps ++text++ in an ins node', () => {
    const ast = doc(para(text('a ++b++ c')));
    insMarkTransform(ast as any);
    const kids = (ast as any).children[0].children;
    expect(kids[0]).toEqual({type: 'text', content: 'a '});
    expect(kids[1]).toEqual({type: 'ins', children: [{type: 'text', content: 'b'}]});
    expect(kids[2]).toEqual({type: 'text', content: ' c'});
  });

  it('wraps ==text== in a mark node', () => {
    const ast = doc(para(text('==hi==')));
    insMarkTransform(ast as any);
    expect((ast as any).children[0].children[0]).toEqual({
      type: 'mark',
      children: [{type: 'text', content: 'hi'}],
    });
  });
});

// ---------------------------------------------------------------------------
describe('definitionListTransform', () => {
  it('converts a term + ":   definition" into a dl', () => {
    const ast = doc(para(text('Coffee')), para(text(':   Black hot drink')));
    definitionListTransform(ast as any);
    const kids = (ast as any).children;
    expect(kids).toHaveLength(1);
    expect(kids[0].type).toBe('dl');
    expect(kids[0].children[0].type).toBe('dt');
    expect(kids[0].children[1].type).toBe('dd');
    expect(getContent(kids[0].children[0])).toBe('Coffee');
    expect(getContent(kids[0].children[1])).toBe('Black hot drink');
  });

  it('leaves ordinary paragraphs alone', () => {
    const ast = doc(para(text('Just a paragraph')));
    definitionListTransform(ast as any);
    expect((ast as any).children[0].type).toBe('paragraph');
  });
});

// ---------------------------------------------------------------------------
describe('abbreviationTransform', () => {
  it('collects the definition and links occurrences', () => {
    const ast = doc(
      para(text('*[HTML]: HyperText Markup Language')),
      para(text('The HTML spec')),
    );
    abbreviationTransform(ast as any);
    const kids = (ast as any).children;
    expect(kids).toHaveLength(1); // definition paragraph removed
    const link = kids[0].children.find((n: any) => n.type === 'link');
    expect(link.href).toBe('#abbr-HTML|HyperText Markup Language');
    expect(getContent(link)).toBe('HTML');
  });

  it('does nothing without definitions', () => {
    const ast = doc(para(text('No abbreviations here')));
    abbreviationTransform(ast as any);
    expect(getContent(ast)).toBe('No abbreviations here');
  });
});

// ---------------------------------------------------------------------------
describe('footnoteTransform', () => {
  it('links a reference and appends a Footnotes section', () => {
    const ast = doc(
      para(text('See note[^1] here.')),
      para(text('[^1]: The footnote text')),
    );
    footnoteTransform(ast as any);
    const kids = (ast as any).children;

    const link = kids[0].children.find((n: any) => n.type === 'link');
    expect(link.href).toBe('#footnote-1@1');
    expect(getContent(link)).toBe('[1]');

    const heading = kids.find((n: any) => n.type === 'heading');
    expect(getContent(heading)).toBe('Footnotes');
    const list = kids.find((n: any) => n.type === 'list');
    expect(list.ordered).toBe(true);
    expect(list.children[0].type).toBe('list_item');
  });

  it('does nothing without footnotes', () => {
    const ast = doc(para(text('Plain text')));
    footnoteTransform(ast as any);
    expect((ast as any).children).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
describe('tableBrTransform', () => {
  it('replaces the placeholder with a newline inside table cells', () => {
    const ast = doc({
      type: 'table',
      children: [{
        type: 'table_row',
        children: [{
          type: 'table_cell',
          children: [text(`line1${TABLE_BR_PLACEHOLDER}line2`)],
        }],
      }],
    });
    tableBrTransform(ast as any);
    // document → table → row → cell → text
    const textNode = (ast as any).children[0].children[0].children[0].children[0];
    expect(textNode.content).toBe('line1\nline2');
  });

  it('leaves the placeholder outside table cells untouched', () => {
    const ast = doc(para(text(`a${TABLE_BR_PLACEHOLDER}b`)));
    tableBrTransform(ast as any);
    expect((ast as any).children[0].children[0].content).toBe(`a${TABLE_BR_PLACEHOLDER}b`);
  });
});
