// Light and friendly sample markdown for Markdownr
export const EXAMPLE_MARKDOWN = `---
title: Welcome to Markdownr
author: The Dude
date: 2025-01-15
tags:
  - markdown
  - mobile
  - documentation
description: A pocket-friendly markdown viewer for iOS and Android
---
# Welcome to Markdownr

Your pocket-friendly markdown viewer for reading docs, notes, and READMEs on the go.

[☰ Table of Contents](markdownr:toc)

## Getting Started

Open any \`.md\` file from your device or cloud storage using the [menu button](markdownr:menu) below. Markdownr will render it beautifully!

### What You Can Do

- [Browse](markdownr:open) your markdown files
- [Search](markdownr:search) within documents
- [Switch](markdownr:theme)
- [Toggle](markdownr:darkmode)

---

## Front Matter Support

Markdownr automatically parses YAML front matter from your documents and displays it as a styled header with tags, title, author, and date.

[Toggle front matter visibility](markdownr:toggle-frontmatter) to show or hide the metadata block. You can also tap the info icon (ⓘ) in the top-right corner.

---

## Theme Support

Documents can suggest a theme via their front matter. Try it out:

[Open Ocean Theme Demo](markdownr:sample-ocean)

---

## Markdown Showcase

Here's a taste of what Markdownr can render:

### Text Formatting

You can write **bold text**, *italics*, or ***both together***. Need to show something removed? Use ~~strikethrough~~.

Inline \`code\` looks great for technical terms like \`useState\` or file names like \`package.json\`.

### Links & Images

Visit the [Markdownr repo](https://github.com) for updates!

![Sample Image](https://picsum.photos/800/400 "Sample image")

### HTML Tags

#### Inline Formatting

Use <b>bold</b> or <strong>strong</strong> with HTML tags.

<i>Italic</i> and <em>emphasis</em> work too.

<s>Strikethrough</s>, <del>deleted</del>, and <strike>struck</strike> text.

<u>Underlined</u> and <ins>inserted</ins> text (rendered as underline).

<mark>Highlighted</mark> text stands out.

Chemical formulas: H<sub>2</sub>O, mathematical: x<sup>2</sup> + y<sup>2</sup>.

Inline HTML <code>code()</code> renders as code.

#### Line Breaks

Paragraph with a hard break<br>right here — using \`<br>\`.

Self-closing variant<br/>also works.

With a space<br />before the slash too.

\`<br>\` also works inside table cells:

| Name | Description |
|------|-------------|
| Alpha | First line<br>second line |
| Beta | One line only |
| Gamma | Line one<br>line two<br>line three |

#### Tags in Code (Preserved)

Inside a code span: \`<b>not bold</b>\` — tags are preserved literally.

\`\`\`html
<b>also preserved</b> in a code block
\`\`\`

### Lists

**Shopping list:**
- Milk
- Eggs
- Coffee beans

**Weekend plans:**
1. Morning run
2. Brunch with friends
3. Read a good book

**Project checklist:**
- [x] Design mockups
- [x] Set up project
- [ ] Write documentation
- [ ] Ship it!

### Blockquotes

> "The Dude abides."
> — Stranger

### Code Blocks

\`\`\`javascript
function greet(name) {
  return \`Hello, \${name}!\`;
}

console.log(greet('Dude'));
\`\`\`

\`\`\`python
def fibonacci(n):
    a, b = 0, 1
    for _ in range(n):
        yield a
        a, b = b, a + b

print(list(fibonacci(10)))
\`\`\`

### Tables

Wide tables scroll horizontally — try swiping left on this one:

| Feature | Notes |
|---------|-------|
| Dark Mode | Follows system setting or manual toggle |
| Search | Full-text search with match highlighting |
| Themes | Default, Sepia, Forest, Ocean, Dracula |
| File Types | Open from Files app or share sheet |
| Math (LaTeX) | Inline and block equations via KaTeX |
| Table of Contents | Auto-generated from headings |
| Pinch to Zoom | Zoom in on any content |
| Double-Tap Zoom | Quick 2x zoom toggle |

### Math Expressions

Einstein's famous equation: $E = mc^2$

The quadratic formula:

$$x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$$

### Nested Content

1. **First topic**
   - Sub-point A
   - Sub-point B
     > A nested quote for emphasis
   - Sub-point C

2. **Second topic**
   - More details here
   - And here too

---

## Tips & Tricks

> **Pro tip:** Swipe up on the menu to reveal more options like theme switching and dark mode toggle!

### Keyboard Shortcuts (on iPad)

| Shortcut | Action |
|----------|--------|
| Cmd + F | Search |
| Cmd + O | Open file |

`;

// Keep the old comprehensive example for reference/testing
export const COMPREHENSIVE_EXAMPLE = `# Nitro Markdown

Welcome to the **high-performance** markdown parser powered by \`md4c\` and **Nitro Modules**.

## Features Showcase

This parser supports *all* the features you'd expect:

- **Bold text** with double asterisks
- *Italic text* with single asterisks
- ~~Strikethrough text~~ (GFM)
- \`Inline code\` snippets
- [Links](https://github.com)
- ![Landscape](https://picsum.photos/seed/markdownr1/300/200 "Sample Image")

**Bold text** and *italic text* and ***bold italic text***.

__Alternative bold__ and _alternative italic_ and ___alternative bold italic___.

~~Strikethrough text~~ and ~~**strikethrough bold**~~ and ~~*strikethrough italic*~~.

Regular text with **bold in the middle** and more text.

A sentence with *multiple* **formatting** ***options*** mixed ~~together~~.

## Some Lists / Tasks

**Quick actions:**

- [ ] Reply to Sarah's email about the \`Series A\` discussion
- [ ] Update your notes on the *TechCrunch* meeting
- [x] Review the [shared document](https://docs.example.com/pitch) before Thursday

**List:**

- Reply to Sarah's email about the \`Series A\` discussion
- Update your notes on the *TechCrunch* meeting
- Review the [shared document](https://docs.example.com/pitch) before Thursday

#### Images
![Landscape](https://picsum.photos/seed/landscape/300/200 "Landscape")
![City](https://picsum.photos/seed/city/300/150 "City")


## Advanced GFM Features

### Task Lists
- [x] Implement md4c parser
- [x] Create Nitro bindings
- [x] Build AST converter
- [ ] Add syntax highlighting
- [ ] Implement caching

### Tables with Complex Content
| Feature | Description | Status | Performance |
|:--------|:------------|:-------|:------------|
| JSI Binding | Direct JS <-> C++ communication | Done | Microseconds |
| Native Threading | Background processing | Done | Optimized |
| Zero-Copy | No data duplication | Done | Memory efficient |
| Math Support | LaTeX expressions | Done | Full featured |
| GFM Tables | Advanced table rendering | Done | Complete spec |

| Name | Email |
|------|-------|
| Alice | alice@example.com |
| Bob | bob@example.com |

## LaTeX Mathematics

### Inline Math
- Simple: $E = mc^2$ with more text after It. Simple: $E = mc^2$ with more text after It.
- Complex: $\\frac{d}{dx}[x^n] = nx^{n-1}$
- Greek letters: $\\alpha + \\beta = \\gamma$
- Subscripts: $x_1, x_2, \\dots, x_n$
- Superscripts: $x^2, y^{n+1}, e^{\\pi i}$

### Block Math (Display Mode)
The quadratic formula:

$$x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$$

Pythagorean theorem:

$$a^2 + b^2 = c^2$$

Matrix operations:

$$\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix} \\times \\begin{pmatrix} x \\\\ y \\end{pmatrix} = \\begin{pmatrix} ax + by \\\\ cx + dy \\end{pmatrix}$$

## Code Blocks with Syntax Highlighting

### TypeScript
\`\`\`typescript
import { parseMarkdown, parseMarkdownWithOptions } from 'react-native-nitro-markdown';

interface ParserOptions {
  gfm?: boolean;
  math?: boolean;
}

const parseWithGFM = (text: string): MarkdownNode => {
  return parseMarkdownWithOptions(text, {
    gfm: true,
    math: true
  });
};
\`\`\`

### C++ (Native Implementation)
\`\`\`cpp
#include "MD4CParser.hpp"

std::shared_ptr<MarkdownNode> parseMarkdown(
    const std::string& text,
    const ParserOptions& options
) {
    MD4CParser parser;
    return parser.parse(text, options);
}
\`\`\`

### Complex Nested Structures

#### Deeply Nested Lists
1. First level ordered item
   - Second level unordered
   - Another second level
     1. Third level ordered
     2. Another third level
        - Fourth level unordered
        - More fourth level items
   - Back to second level
2. Second first level item
   1. Nested ordered in second item
   2. Another nested ordered

#### Blockquotes Within Lists
1. First item with blockquote:
   > This is a blockquote inside a list item
   >
   > It can span multiple lines
   > And contain **formatting**

2. Second item
   - Nested bullet with blockquote:
     > Another blockquote
     > With multiple paragraphs
     >
     > And even more content

### Multi-line Blockquote

> This blockquote spans multiple lines.
> It continues here on the second line.
> And even a third line for good measure.

### Blockquote with Formatting

> **Important:** This blockquote contains *formatted* text.
> It also has \`inline code\` and a [link](https://example.com).

### Text After Blockquote

> A quote about something meaningful.

Regular paragraph text that follows the blockquote.

## Horizontal Rules and Separators

Content above first rule

---

Content between rules

***

More content between rules

___

Content below rules

## Unicode and International Content

### Multiple Languages
- English: Hello world!
- Espanol: Hola mundo!
- Francais: Bonjour le monde!
- Deutsch: Hallo Welt!
- Chinese: Ni hao!
- Japanese: Konnichiwa!
- Arabic: Marhaba!

### Special Characters and Symbols
- Mathematical: + - x / = < >
- Arrows: <- ->
- Currency: $ EUR JPY GBP
- Legal: (c) (R) TM
- Fractions: 1/2 1/3 1/4 3/4

## Performance Test Patterns

### Repeated Patterns
**Bold text** repeated for *performance testing* with \`code blocks\` and [links](url) to ensure the parser handles repetition efficiently without memory leaks or performance degradation.

### Large Content Sections
This section contains intentionally large blocks of content to test how well the parser scales with document size. The content includes various markdown elements mixed together in realistic patterns that would appear in actual documentation or blog posts.

By including diverse content types - headings, paragraphs, lists, tables, code blocks, math expressions, and international text - we create a comprehensive test that exercises all aspects of the markdown parsing engine.

The goal is to ensure that performance remains consistent regardless of content complexity or document length, providing users with reliable and fast markdown processing capabilities.

### Stress Testing Elements
- Multiple consecutive code blocks
- Tables with many columns and rows
- Deeply nested list structures
- Complex mathematical expressions
- Mixed inline formatting combinations
- Large blocks of plain text
- Unicode characters from multiple languages
- Special symbols and emoji combinations

This comprehensive test suite validates that the parser maintains high performance and accuracy across all supported markdown features and edge cases.

## HTML Tags

### Inline Formatting

Use <b>bold</b> or <strong>strong</strong> with HTML tags.

<i>Italic</i> and <em>emphasis</em> work too.

<s>Strikethrough</s>, <del>deleted</del>, and <strike>struck</strike> text.

<u>Underlined</u> and <ins>inserted</ins> text.

<mark>Highlighted</mark> text.

H<sub>2</sub>O and x<sup>2</sup>.

Inline HTML <code>code()</code>.

### Line Breaks

Paragraph with a hard break<br>right here — using \`<br>\`.

Self-closing variant<br/>also works.

With a space<br />before the slash too.`;

export const OCEAN_THEMED_SAMPLE = `---
title: Ocean Theme Demo
author: Markdownr
theme: ocean
tags:
  - themes
  - demo
---
# Exploring Ocean Theme

This document includes \`theme: ocean\` in its front matter. You should see a banner at the top suggesting the Ocean theme.

Tap **Apply** to switch, or **Dismiss** to keep your current theme.

---

## How It Works

Any markdown file can include a \`theme\` key in its YAML front matter:

\`\`\`yaml
---
theme: ocean
---
\`\`\`

### Built-in Themes

You can use any built-in theme name:

| Theme | Emoji | Style |
|-------|-------|-------|
| default | ⚪ | Clean and minimal |
| ocean | 🌊 | Cool blues and teals |
| forest | 🌲 | Earthy greens |
| sunset | 🌅 | Warm oranges and reds |
| lavender | 💜 | Soft purples |

### Remote Themes

You can also point to a remote JSON theme:

\`\`\`yaml
---
theme: https://example.com/my-theme.json
---
\`\`\`

Remote themes are fetched, validated, and cached for offline use. They can even include custom fonts!

---

> "The sea, once it casts its spell, holds one in its net of wonder forever."
> — Jacques Cousteau

---

[Back to Welcome](markdownr:home)
`;

export const CUSTOM_RENDER_COMPONENTS = `# Custom Renderer Examples

> **Tip:** Use the bottom tabs to switch between rendering modes!
>
> - **Default:** Standard markdown rendering
> - **Styles:** Custom accents and retro typography
> - **Custom:** Completely replaced components (Cards, Alerts, etc.)

## Custom Components Demo

This image will look like a standard image in **Default**, but like a "Card" with shadow in **Custom**:

![Demo Image](https://picsum.photos/seed/demo/800/400 "A sample demo image")

This blockquote will look like a gray bar in **Default**, but like an "Alert Info" box in **Custom**:

> **Did you know?**
>
> The Custom renderer replaces the standard \`View\` with a specialized component that includes an icon and different layout logic!

---`;
