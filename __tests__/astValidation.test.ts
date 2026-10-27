import {describe, it, expect} from 'vitest';
// The real validator nitro-markdown runs on astTransform output (the package
// alias points at a mock, so import the source file directly).
import {cloneMarkdownNode} from '../node_modules/react-native-nitro-markdown/src/utils/freeze-ast';
import {createViewerTransform, customNodeKind} from '../astTransforms';

const doc = (...children: any[]) => ({type: 'document', children});
const para = (...children: any[]) => ({type: 'paragraph', children});
const text = (content: string) => ({type: 'text', content});
const blockquote = (...children: any[]) => ({type: 'blockquote', children});

function sampleDocument() {
  return doc(
    para(text('*[HTML]: HyperText Markup Language')),
    para(text('"Quoted" HTML with ++inserted++, ==marked==, x^2^ and H~2~O :-)')),
    para(text('See note[^1] here.')),
    para(text('Coffee')),
    para(text(':   Black hot drink')),
    para(text('Tea'), {type: 'soft_break'}, text('  ~ Green or black')),
    para(text(':::quotes')),
    blockquote(para(text('Quote one'))),
    blockquote(para(text('Quote two'))),
    para(text(':::')),
    para(text('[^1]: The footnote text')),
  );
}

describe('viewer transform output', () => {
  it('passes nitro-markdown AST validation', () => {
    const ast = createViewerTransform('en')(sampleDocument() as any);
    expect(() => cloneMarkdownNode(ast)).not.toThrow();
  });

  it('covers every custom node kind', () => {
    const ast = createViewerTransform('en')(sampleDocument() as any);
    const kinds = new Set<string>();
    const walk = (node: any) => {
      const kind = customNodeKind(node);
      if (kind) kinds.add(kind);
      node.children?.forEach(walk);
    };
    walk(ast);
    expect([...kinds].sort()).toEqual(['dd', 'dl', 'dt', 'ins', 'mark', 'quote_cycle']);
  });

  it('would be rejected if it used unknown node types', () => {
    expect(() => cloneMarkdownNode(doc({type: 'mark', children: []}) as any))
      .toThrow(/unknown AST node type/);
  });
});
