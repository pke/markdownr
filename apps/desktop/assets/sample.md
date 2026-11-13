---
title: Markdownr Prototype
theme: default
---
# Markdownr in Rust

This is the **Direct2D + DirectWrite** prototype (ClearType): pulldown-cmark → own AST → *transforms* → layout → native text engine.
Select text across ~~one~~ several paragraphs with the mouse and press `Ctrl+C`. :rocket:

Extra syntax from transforms: ==marked text==, ++inserted text++, x^2^ and H~2~O, emoticons :)

## Lists

- First item with a [link](https://github.com/pke/markdownr)
- Second item
  - Nested item
  - Another nested item
- [x] A finished task
- [ ] An open task

1. Ordered one
2. Ordered two
3. Ordered three

## Quotes and alerts

> A classic quote. Reading should feel calm and focused,
> with generous line height and a comfortable column width.

> [!NOTE]
> GitHub-style alerts are supported by pulldown-cmark out of the box.

> [!WARNING]
> Images, syntax highlighting and math are still placeholders in this prototype.

## Code

```rust
fn main() {
    let doc = markdownr::parse("# Hello");
    println!("{doc:?}");
}
```

## Table

| Feature | Status | Notes |
|:--------|:------:|------:|
| Parser | ✅ | pulldown-cmark 0.13 |
| Layout | ✅ | DirectWrite |
| Rendering | ✅ | Direct2D, ClearType |
| Selection | ✅ | across blocks |
| Images | ⏳ | next step |

## Definition list

Reader
: An app that shows documents, it does not edit them.

GPU
: Graphics processing unit.

## Footnotes and math

Here is a footnote reference[^1] and inline math $e^{i\pi} + 1 = 0$.

$$
\int_0^1 x^2 \, dx = \frac{1}{3}
$$

---

[^1]: The footnote text lives at the bottom.

## Keyboard

Page Up / Page Down / Space / arrow keys scroll, Home / End jump, **F11** toggles full screen, **Esc** leaves it,
`Ctrl +` / `Ctrl -` / `Ctrl 0` change the font size, `Ctrl+A` selects everything. Drop a `.md` file onto the window to open it.

## A long section to scroll

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer posuere erat a ante venenatis dapibus posuere velit aliquet. Cras mattis consectetur purus sit amet fermentum. Donec ullamcorper nulla non metus auctor fringilla.

Vestibulum id ligula porta felis euismod semper. Maecenas faucibus mollis interdum. Nullam quis risus eget urna mollis ornare vel eu leo. Curabitur blandit tempus porttitor.

Sed posuere consectetur est at lobortis. Aenean eu leo quam. Pellentesque ornare sem lacinia quam venenatis vestibulum. Etiam porta sem malesuada magna mollis euismod.
