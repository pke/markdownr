// @ts-ignore - importing from source to use patched astTransform
import type {MarkdownNode} from 'react-native-nitro-markdown/src';

type AstTransform = (ast: MarkdownNode) => MarkdownNode;

/** Compose multiple AST transforms into a single transform, applied left-to-right. */
export function composeTransforms(...transforms: AstTransform[]): AstTransform {
  return (ast: MarkdownNode) => transforms.reduce((a, fn) => fn(a), ast);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const SKIP_TRANSFORM = new Set([
  'table', 'table_head', 'table_body', 'table_row', 'table_cell',
  'code_block', 'code_inline',
]);

function walkTextNodes(
  node: MarkdownNode,
  fn: (node: MarkdownNode) => void,
): void {
  if (SKIP_TRANSFORM.has(node.type)) return;
  if (node.type === 'text' && node.content) fn(node);
  node.children?.forEach((child: MarkdownNode) => walkTextNodes(child, fn));
}

/** Recursively extract all text content from a node tree. */
function getAllText(node: MarkdownNode): string {
  if (node.content) return node.content;
  return node.children?.map(getAllText).join('') ?? '';
}

/**
 * Strip the "[^label]: " prefix from the first text node in a children array.
 * Returns a new array with the prefix removed, preserving all other AST nodes.
 */
function stripDefPrefix(
  children: MarkdownNode[],
  label: string,
): MarkdownNode[] {
  const prefix = `[^${label}]: `;
  const result: MarkdownNode[] = [];
  let stripped = false;
  for (const child of children) {
    if (!stripped && child.type === 'text' && child.content) {
      const idx = child.content.indexOf(prefix);
      if (idx !== -1) {
        const after = child.content.slice(idx + prefix.length);
        if (after) {
          result.push({type: 'text', content: after});
        }
        stripped = true;
        continue;
      }
    }
    result.push(child);
  }
  return result;
}

/**
 * Walk the tree and expand text nodes that match `pattern` into a mix of
 * text nodes and nodes returned by `replacer`. The replacer receives each
 * regex match and returns a MarkdownNode (typically a link).
 * Non-matching segments become plain text nodes.
 */
function spliceTextNodes(
  node: MarkdownNode,
  pattern: RegExp,
  replacer: (match: RegExpExecArray) => MarkdownNode,
): void {
  if (!node.children || SKIP_TRANSFORM.has(node.type)) return;
  const newChildren: MarkdownNode[] = [];
  for (const child of node.children) {
    if (child.type === 'text' && child.content) {
      const parts = splitByPattern(child.content, pattern, replacer);
      newChildren.push(...parts);
    } else {
      spliceTextNodes(child, pattern, replacer);
      newChildren.push(child);
    }
  }
  node.children = newChildren;
}

function splitByPattern(
  text: string,
  pattern: RegExp,
  replacer: (match: RegExpExecArray) => MarkdownNode,
): MarkdownNode[] {
  const result: MarkdownNode[] = [];
  pattern.lastIndex = 0;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) {
      result.push({type: 'text', content: text.slice(lastIndex, match.index)});
    }
    result.push(replacer(match));
    lastIndex = pattern.lastIndex;
  }
  if (lastIndex < text.length) {
    result.push({type: 'text', content: text.slice(lastIndex)});
  }
  return result.length > 0 ? result : [{type: 'text', content: text}];
}

// ---------------------------------------------------------------------------
// Typographic replacements   (c) → ©   "quotes" → "quotes"   -- → –
// ---------------------------------------------------------------------------

type Replacement = string | ((m: string, ...args: string[]) => string);

// Locale → [openDouble, closeDouble, openSingle, closeSingle]
type QuoteSet = [string, string, string, string];
const quoteStyles = new Map<string, QuoteSet>([
  ['en', ['\u201C', '\u201D', '\u2018', '\u2019']],  // " " ' '
  ['de', ['\u201E', '\u201C', '\u201A', '\u2018']],  // „ " ‚ '
  ['fr', ['\u00AB\u00A0', '\u00A0\u00BB', '\u2039\u00A0', '\u00A0\u203A']],  // « » ‹ ›
  ['ru', ['\u00AB', '\u00BB', '\u201E', '\u201C']],   // « » „ "
  ['ja', ['\u300C', '\u300D', '\u300E', '\u300F']],   // 「 」 『 』
  ['zh', ['\u201C', '\u201D', '\u2018', '\u2019']],   // " " ' '
  ['pl', ['\u201E', '\u201D', '\u201A', '\u2019']],   // „ " ‚ '
  ['es', ['\u00AB', '\u00BB', '\u201C', '\u201D']],   // « » " "
  ['it', ['\u00AB', '\u00BB', '\u201C', '\u201D']],   // « » " "
  ['pt', ['\u201C', '\u201D', '\u2018', '\u2019']],   // " " ' '
  ['nl', ['\u201C', '\u201D', '\u2018', '\u2019']],   // " " ' '
  ['sv', ['\u201D', '\u201D', '\u2019', '\u2019']],   // " " ' '
  ['da', ['\u201E', '\u201C', '\u201A', '\u2018']],   // „ " ‚ '
  ['cs', ['\u201E', '\u201C', '\u201A', '\u2018']],   // „ " ‚ '
  ['hu', ['\u201E', '\u201D', '\u201A', '\u2019']],   // „ " ‚ '
]);

const defaultQuotes: QuoteSet = ['\u201C', '\u201D', '\u2018', '\u2019'];

function getQuotes(locale: string): QuoteSet {
  const lang = locale.split(/[-_]/)[0].toLowerCase();
  return quoteStyles.get(lang) ?? defaultQuotes;
}

function buildTypographicRules(locale: string): [RegExp, Replacement][] {
  const [oD, cD, oS, cS] = getQuotes(locale);
  return [
    // Order matters: longest patterns first to avoid partial matches
    // em-dash: --- but not ---- or more
    [/(^|[^-])---(?=[^-]|$)/gm, '$1\u2014'],
    // en-dash: -- between spaces, or between non-dash non-space chars
    [/(^|\s)--(?=\s|$)/gm, '$1\u2013'],
    [/(^|[^-\s])--(?=[^-\s]|$)/gm, '$1\u2013'],
    // ellipsis: .., ..., ....... → … but ?..... → ?.. and !..... → !..
    [/\.{2,}/g, '\u2026'],
    [/([?!])\u2026/g, '$1..'],
    [/\+-/g, '\u00B1'],           // plus-minus
    [/\(c\)/gi, '\u00A9'],        // copyright
    [/\(r\)/gi, '\u00AE'],        // registered
    [/\(tm\)/gi, '\u2122'],       // trademark
    [/\(p\)/gi, '\u00A7'],        // section sign
    [/([?!])\1{3,}/g, (_m: string, ch: string) => ch.repeat(3)],  // collapse ???? → ???
    [/,{2,}/g, ','],              // collapse ,,, → ,
    // Smart double quotes (locale-aware)
    [/(?<=^|[\s(\[{])"(?=\S)/gm, oD],   // opening "
    [/"(?=[\s)\]}.,:;!?]|$)/gm, cD],     // closing "
    [/"/g, cD],                           // remaining " → closing
    // Smart single quotes (locale-aware)
    [/(?<=^|[\s(\[{])'(?=\S)/gm, oS],   // opening '
    [/'(?=[\s)\]}.,:;!?]|$)/gm, cS],     // closing '
    [/'/g, cS],                           // remaining ' → closing (also apostrophe)
  ];
}

export function createTypographicTransform(locale: string): AstTransform {
  const rules = buildTypographicRules(locale);
  return (ast: MarkdownNode) => {
    walkTextNodes(ast, node => {
      let text = node.content!;
      for (const [pattern, replacement] of rules) {
        if (typeof replacement === 'string') {
          text = text.replace(pattern, replacement);
        } else {
          text = text.replace(pattern, replacement as (...args: string[]) => string);
        }
      }
      node.content = text;
    });
    return ast;
  };
}

// ---------------------------------------------------------------------------
// Emoji transform   :grin: → 😁   :-) → 😃
//   Uses full markdown-it-emoji dataset (1900+ shortcodes) + emoticon shortcuts
// ---------------------------------------------------------------------------

import { emojiData } from './emojiData';

// Emoticon shortcuts → emoji name (from markdown-it-emoji shortcuts.mjs)
const shortcutEntries: [string, string][] = [
  ['>:(', 'angry'], ['>:-(', 'angry'],
  [':")', 'blush'], [':-")', 'blush'],
  ['</3', 'broken_heart'], ['<\\3', 'broken_heart'],
  [':/', 'confused'], [':-/', 'confused'],
  [":'(", 'cry'], [":'-(", 'cry'], [':,(', 'cry'], [':,-(', 'cry'],
  [':(', 'frowning'], [':-(', 'frowning'],
  ['<3', 'heart'],
  [']:(', 'imp'], [']:-(', 'imp'],
  ['o:)', 'innocent'], ['O:)', 'innocent'], ['o:-)', 'innocent'],
  ['O:-)', 'innocent'], ['0:)', 'innocent'], ['0:-)', 'innocent'],
  [":')", 'joy'], [":'-)", 'joy'], [':,)', 'joy'], [':,-)', 'joy'],
  [":'D", 'joy'], [":'-D", 'joy'], [':,D', 'joy'], [':,-D', 'joy'],
  [':*', 'kissing'], [':-*', 'kissing'],
  ['x-)', 'laughing'], ['X-)', 'laughing'],
  [':|', 'neutral_face'], [':-|', 'neutral_face'],
  [':o', 'open_mouth'], [':-o', 'open_mouth'], [':O', 'open_mouth'], [':-O', 'open_mouth'],
  [':@', 'rage'], [':-@', 'rage'],
  [':D', 'smile'], [':-D', 'smile'],
  [':)', 'smiley'], [':-)', 'smiley'],
  [']:)', 'smiling_imp'], [']:-)','smiling_imp'],
  [":,'(", 'sob'], [":,'-(", 'sob'], [';(', 'sob'], [';-(', 'sob'],
  [':P', 'stuck_out_tongue'], [':-P', 'stuck_out_tongue'],
  ['8-)', 'sunglasses'], ['B-)', 'sunglasses'],
  [',:(', 'sweat'], [',:(', 'sweat'],
  [',:)', 'sweat_smile'], [',:-)','sweat_smile'],
  [':s', 'unamused'], [':-S', 'unamused'], [':z', 'unamused'],
  [':-Z', 'unamused'], [':$', 'unamused'], [':-$', 'unamused'],
  [';)', 'wink'], [';-)', 'wink'],
];

// Build shortcut map: emoticon text → emoji character
const shortcutMap = new Map<string, string>();
for (const [text, name] of shortcutEntries) {
  const emoji = emojiData.get(name);
  if (emoji) shortcutMap.set(text, emoji);
}

// Single regex for emoticon shortcuts (longest first to avoid partial matches)
const shortcutPattern = new RegExp(
  [...shortcutMap.keys()]
    .sort((a, b) => b.length - a.length)
    .map(k => k.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
    .join('|'),
  'g',
);

// Fast regex: match :shortcode: patterns (letters, digits, underscores, +, -)
const shortcodePattern = /:([a-z0-9_+-]+):/g;

export function emoticonTransform(ast: MarkdownNode): MarkdownNode {
  walkTextNodes(ast, node => {
    let text = node.content!;
    // Pass 1: :shortcode: → emoji via Map lookup
    text = text.replace(shortcodePattern, (match: string, name: string) => emojiData.get(name) ?? match);
    // Pass 2: emoticon shortcuts → emoji
    text = text.replace(shortcutPattern, (match: string) => shortcutMap.get(match) ?? match);
    node.content = text;
  });
  return ast;
}

// ---------------------------------------------------------------------------
// Sub/superscript transform   ^th^  →  ᵗʰ   ~2~  →  ₂
// ---------------------------------------------------------------------------

const superMap = new Map<string, string>([
  ['0', '⁰'], ['1', '¹'], ['2', '²'], ['3', '³'], ['4', '⁴'],
  ['5', '⁵'], ['6', '⁶'], ['7', '⁷'], ['8', '⁸'], ['9', '⁹'],
  ['+', '⁺'], ['-', '⁻'], ['=', '⁼'], ['(', '⁽'], [')', '⁾'],
  ['a', 'ᵃ'], ['b', 'ᵇ'], ['c', 'ᶜ'], ['d', 'ᵈ'], ['e', 'ᵉ'],
  ['f', 'ᶠ'], ['g', 'ᵍ'], ['h', 'ʰ'], ['i', 'ⁱ'], ['j', 'ʲ'],
  ['k', 'ᵏ'], ['l', 'ˡ'], ['m', 'ᵐ'], ['n', 'ⁿ'], ['o', 'ᵒ'],
  ['p', 'ᵖ'], ['r', 'ʳ'], ['s', 'ˢ'], ['t', 'ᵗ'], ['u', 'ᵘ'],
  ['v', 'ᵛ'], ['w', 'ʷ'], ['x', 'ˣ'], ['y', 'ʸ'], ['z', 'ᶻ'],
  ['A', 'ᴬ'], ['B', 'ᴮ'], ['D', 'ᴰ'], ['E', 'ᴱ'], ['G', 'ᴳ'],
  ['H', 'ᴴ'], ['I', 'ᴵ'], ['J', 'ᴶ'], ['K', 'ᴷ'], ['L', 'ᴸ'],
  ['M', 'ᴹ'], ['N', 'ᴺ'], ['O', 'ᴼ'], ['P', 'ᴾ'], ['R', 'ᴿ'],
  ['T', 'ᵀ'], ['U', 'ᵁ'], ['V', 'ⱽ'], ['W', 'ᵂ'],
]);

const subMap = new Map<string, string>([
  ['0', '₀'], ['1', '₁'], ['2', '₂'], ['3', '₃'], ['4', '₄'],
  ['5', '₅'], ['6', '₆'], ['7', '₇'], ['8', '₈'], ['9', '₉'],
  ['+', '₊'], ['-', '₋'], ['=', '₌'], ['(', '₍'], [')', '₎'],
  ['a', 'ₐ'], ['e', 'ₑ'], ['h', 'ₕ'], ['i', 'ᵢ'], ['j', 'ⱼ'],
  ['k', 'ₖ'], ['l', 'ₗ'], ['m', 'ₘ'], ['n', 'ₙ'], ['o', 'ₒ'],
  ['p', 'ₚ'], ['r', 'ᵣ'], ['s', 'ₛ'], ['t', 'ₜ'], ['u', 'ᵤ'],
  ['v', 'ᵥ'], ['x', 'ₓ'],
]);

const toSuper = (s: string) => [...s].map(c => superMap.get(c) ?? c).join('');
const toSub = (s: string) => [...s].map(c => subMap.get(c) ?? c).join('');
const subSuperPattern = /\^([^^]+)\^|~([^~]+)~/g;

export function subSuperscriptTransform(ast: MarkdownNode): MarkdownNode {
  walkTextNodes(ast, node => {
    node.content = node.content!.replace(
      subSuperPattern,
      (_: string, sup: string, sub: string) =>
        sup ? toSuper(sup) : toSub(sub),
    );
  });
  return ast;
}

// ---------------------------------------------------------------------------
// Insert / Mark transforms   ++inserted++ → <ins>   ==marked== → <mark>
// ---------------------------------------------------------------------------

const insPattern = /\+\+([^+]+)\+\+/g;
const markPattern = /==([^=]+)==/g;

export function insMarkTransform(ast: MarkdownNode): MarkdownNode {
  spliceTextNodes(ast, insPattern, (match) => ({
    type: 'ins' as MarkdownNode['type'],
    children: [{type: 'text', content: match[1]}],
  }));
  spliceTextNodes(ast, markPattern, (match) => ({
    type: 'mark' as MarkdownNode['type'],
    children: [{type: 'text', content: match[1]}],
  }));
  return ast;
}

// ---------------------------------------------------------------------------
// Definition list transform
//   Term 1
//   :   Definition 1        (colon + 3 spaces)
//   Term 2
//     ~ Definition 2a       (2 spaces + tilde + space)  compact style
// ---------------------------------------------------------------------------

const defListColonRe = /^:\s{3}/;
const defListTildeRe = /^\s{0,2}~\s/;

function isDefParagraph(node: MarkdownNode): boolean {
  if (node.type !== 'paragraph' || !node.children) return false;
  const first = node.children[0];
  if (!first || first.type !== 'text' || !first.content) return false;
  return defListColonRe.test(first.content) || defListTildeRe.test(first.content);
}

function stripDefMarker(node: MarkdownNode): MarkdownNode[] {
  if (!node.children) return [];
  const children = [...node.children];
  const first = children[0];
  if (first?.type === 'text' && first.content) {
    const colonMatch = first.content.match(/^:\s{3}(.*)/s);
    const tildeMatch = first.content.match(/^\s{0,2}~\s(.*)/s);
    const rest = colonMatch?.[1] ?? tildeMatch?.[1] ?? first.content;
    if (rest) {
      children[0] = {...first, content: rest};
    } else {
      children.shift();
    }
  }
  return children;
}

export function definitionListTransform(ast: MarkdownNode): MarkdownNode {
  if (!ast.children) return ast;

  function processChildren(parent: MarkdownNode): void {
    if (!parent.children) return;

    const newChildren: MarkdownNode[] = [];
    let i = 0;

    while (i < parent.children.length) {
      const child = parent.children[i];

      // Check if next sibling is a definition paragraph
      if (child.type === 'paragraph' && !isDefParagraph(child) &&
          i + 1 < parent.children.length && isDefParagraph(parent.children[i + 1])) {
        // This paragraph is a term, collect definitions
        const dtNode: MarkdownNode = {
          type: 'dt' as MarkdownNode['type'],
          children: child.children,
        };

        const ddNodes: MarkdownNode[] = [];
        i++;
        while (i < parent.children.length) {
          const next = parent.children[i];
          if (isDefParagraph(next)) {
            // New `:   ` or `~ ` definition
            ddNodes.push({
              type: 'dd' as MarkdownNode['type'],
              children: stripDefMarker(next),
            });
            i++;
          } else if (ddNodes.length > 0 && next.type === 'code_block') {
            // Continuation code block for the last dd (indented code)
            const lastDd = ddNodes[ddNodes.length - 1];
            lastDd.children = [...(lastDd.children ?? []), next];
            i++;
          } else {
            break;
          }
        }

        newChildren.push({
          type: 'dl' as MarkdownNode['type'],
          children: [dtNode, ...ddNodes],
        });
      } else {
        // Also check for compact style: single paragraph with embedded ~ definitions
        if (child.type === 'paragraph' && child.children) {
          const compactDl = tryCompactDefList(child);
          if (compactDl) {
            newChildren.push(compactDl);
            i++;
            continue;
          }
        }
        processChildren(child);
        newChildren.push(child);
        i++;
      }
    }

    parent.children = newChildren;
  }

  processChildren(ast);
  return ast;
}

function tryCompactDefList(para: MarkdownNode): MarkdownNode | null {
  if (!para.children) return null;
  // Compact style: "Term\n  ~ Def1\n  ~ Def2" becomes a single paragraph
  // with text nodes and soft_break nodes. Look for soft_break followed by
  // a text node starting with ~
  let hasTildeDef = false;
  for (let i = 0; i < para.children.length; i++) {
    const c = para.children[i];
    if (c.type === 'soft_break' && i + 1 < para.children.length) {
      const next = para.children[i + 1];
      if (next.type === 'text' && next.content && defListTildeRe.test(next.content)) {
        hasTildeDef = true;
        break;
      }
    }
  }
  if (!hasTildeDef) return null;

  // Split children at soft_break boundaries
  const lines: MarkdownNode[][] = [[]];
  for (const c of para.children) {
    if (c.type === 'soft_break') {
      lines.push([]);
    } else {
      lines[lines.length - 1].push(c);
    }
  }

  // First line(s) without ~ prefix are the term, rest are definitions
  const termChildren: MarkdownNode[] = [];
  const ddNodes: MarkdownNode[] = [];
  let inDefs = false;

  for (const line of lines) {
    const first = line[0];
    if (first?.type === 'text' && first.content && defListTildeRe.test(first.content)) {
      inDefs = true;
      const match = first.content.match(/^\s{0,2}~\s(.*)/s);
      const rest = match?.[1] ?? first.content;
      const stripped = rest ? [{...first, content: rest}, ...line.slice(1)] : line.slice(1);
      ddNodes.push({
        type: 'dd' as MarkdownNode['type'],
        children: stripped,
      });
    } else if (!inDefs) {
      if (termChildren.length > 0) {
        termChildren.push({type: 'soft_break'} as MarkdownNode);
      }
      termChildren.push(...line);
    }
  }

  if (termChildren.length === 0 || ddNodes.length === 0) return null;

  return {
    type: 'dl' as MarkdownNode['type'],
    children: [
      {type: 'dt' as MarkdownNode['type'], children: termChildren},
      ...ddNodes,
    ],
  };
}

// ---------------------------------------------------------------------------
// Abbreviation transform   *[HTML]: Hyper Text Markup Language
//   Collects definitions, removes them, replaces occurrences with link nodes
//   using href="#abbr-TERM|Full expansion" for rendering as dotted-underline.
// ---------------------------------------------------------------------------

const abbrDefPattern = /^\*\[([^\]]+)\]:\s*(.+)$/;

export function abbreviationTransform(ast: MarkdownNode): MarkdownNode {
  const abbrDefs = new Map<string, string>();

  // Pass 1: collect definitions and remove them from tree
  function collectAbbrDefs(node: MarkdownNode): void {
    if (!node.children) return;
    node.children = node.children.filter((child: MarkdownNode) => {
      if (child.type === 'paragraph' && child.children) {
        const text = getAllText(child).trim();
        const match = text.match(abbrDefPattern);
        if (match) {
          abbrDefs.set(match[1], match[2]);
          return false; // remove definition paragraph
        }
      }
      collectAbbrDefs(child);
      return true;
    });
  }
  collectAbbrDefs(ast);

  if (abbrDefs.size === 0) return ast;

  // Pass 2: replace occurrences with link nodes carrying abbreviation data
  const abbrKeys = [...abbrDefs.keys()]
    .sort((a, b) => b.length - a.length) // longest first
    .map(k => k.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const abbrPattern = new RegExp(`\\b(${abbrKeys.join('|')})\\b`, 'g');

  spliceTextNodes(ast, abbrPattern, (match) => {
    const term = match[1];
    const full = abbrDefs.get(term) ?? term;
    return {
      type: 'link',
      href: `#abbr-${term}|${full}`,
      children: [{type: 'text', content: term}],
    };
  });

  return ast;
}

// ---------------------------------------------------------------------------
// Footnote transform (bidirectional links)
//   [^label]         →  link to #footnote-N      (superscript N)
//   ^[inline text]   →  link to #footnote-N      (superscript N)
//   [^label]: text   →  collected, removed, rendered in Footnotes section
//   Each footnote item has a back-link to #footnote-ref-N
// ---------------------------------------------------------------------------

const defPattern = /^\[\^([^\]]+)\]:\s*(.+)$/;
// Combined pattern: matches inline ^[text] (group 1) or reference [^label] (group 2)
const footnotePattern = /\^\[([^\]]+)\]|\[\^([^\]]+)\]/g;

export function footnoteTransform(ast: MarkdownNode): MarkdownNode {
  // Store footnote content as AST node arrays to preserve markup
  const footnoteDefs = new Map<string, MarkdownNode[]>();
  const footnoteOrder: string[] = [];
  const refCounts = new Map<number, number>();
  let inlineCount = 0;

  // Pass 1: collect definitions (preserving AST nodes) and remove them from tree
  function collectDefs(node: MarkdownNode): void {
    if (!node.children) return;
    const newChildren: MarkdownNode[] = [];
    let lastDefLabel: string | null = null;
    for (const child of node.children) {
      if (child.type === 'paragraph' && child.children) {
        const fullText = getAllText(child).trim();
        const match = fullText.match(defPattern);
        if (match) {
          const label = match[1];
          // Strip the "[^label]: " prefix from the first text node, keep rest as AST
          const contentChildren = stripDefPrefix(child.children, label);
          footnoteDefs.set(label, [{
            type: 'paragraph' as const,
            children: contentChildren,
          }]);
          lastDefLabel = label;
          continue;
        }
        // Continuation paragraph after a definition
        if (lastDefLabel) {
          const existing = footnoteDefs.get(lastDefLabel) ?? [];
          existing.push(child);
          footnoteDefs.set(lastDefLabel, existing);
          continue;
        }
      } else if (lastDefLabel && child.type === 'code_block') {
        // 4-space indented continuation → convert to paragraph for display
        const existing = footnoteDefs.get(lastDefLabel) ?? [];
        const text = getAllText(child).trim();
        existing.push({
          type: 'paragraph' as const,
          children: [{type: 'text' as const, content: text}],
        });
        footnoteDefs.set(lastDefLabel, existing);
        continue;
      } else {
        lastDefLabel = null;
      }
      collectDefs(child);
      newChildren.push(child);
    }
    node.children = newChildren;
  }
  collectDefs(ast);

  // Pass 2: replace both inline and reference footnotes in document order
  spliceTextNodes(ast, footnotePattern, (match) => {
    const inlineText = match[1];
    const refLabel = match[2];

    if (inlineText) {
      const key = `__inline_${inlineCount++}`;
      footnoteDefs.set(key, [{
        type: 'paragraph' as const,
        children: [{type: 'text' as const, content: inlineText}],
      }]);
      footnoteOrder.push(key);
    } else if (refLabel) {
      if (!footnoteOrder.includes(refLabel)) {
        footnoteOrder.push(refLabel);
      }
    }

    const num = inlineText
      ? footnoteOrder.length
      : footnoteOrder.indexOf(refLabel!) + 1;

    const occIdx = (refCounts.get(num) ?? 0) + 1;
    refCounts.set(num, occIdx);

    return {
      type: 'link',
      href: `#footnote-${num}@${occIdx}`,
      children: [{type: 'text', content: `[${num}]`}],
    };
  });

  // Pass 3: append footnotes section with back-links
  if (footnoteOrder.length > 0) {
    const footnoteItems: MarkdownNode[] = footnoteOrder.map((label, i) => {
      const num = i + 1;
      const contentNodes = footnoteDefs.get(label) ?? [
        {type: 'paragraph' as const, children: [{type: 'text' as const, content: label}]},
      ];
      const count = refCounts.get(num) ?? 1;
      const backLinks: MarkdownNode[] = [];
      for (let occ = 1; occ <= count; occ++) {
        if (occ > 1) {
          backLinks.push({type: 'text' as const, content: ' '});
        }
        backLinks.push({
          type: 'link' as const,
          href: `#footnote-ref-${num}-${occ}`,
          children: [{
            type: 'text' as const,
            content: count > 1 ? `\u21a9${toSuper(String(occ))}` : '\u21a9',
          }],
        });
      }
      // Append back-links to the last paragraph's children
      const items = [...contentNodes];
      const lastPara = items[items.length - 1];
      if (lastPara && lastPara.type === 'paragraph' && lastPara.children) {
        lastPara.children = [
          ...lastPara.children,
          {type: 'text' as const, content: ' '},
          ...backLinks,
        ];
      } else {
        items.push({
          type: 'paragraph' as const,
          children: backLinks,
        });
      }
      return {
        type: 'list_item' as const,
        children: items,
      };
    });
    ast.children = ast.children ?? [];
    ast.children.push(
      {type: 'horizontal_rule' as const},
      {
        type: 'heading' as const,
        level: 4,
        children: [{type: 'text' as const, content: 'Footnotes'}],
      },
      {
        type: 'list' as const,
        ordered: true,
        start: 1,
        children: footnoteItems,
      },
    );
  }

  return ast;
}
