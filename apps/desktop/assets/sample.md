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
> Remote images load in the background; SVG is not supported yet.

## Images

![Sample Image](https://picsum.photos/800/400 "Sample image")

## Code

```rust
/// Parse and print a document.
fn main() {
    let doc = markdownr::parse("# Hello");
    println!("{doc:?} {}", 42);
}
```

```json
{ "name": "markdownr", "version": "0.1.0", "native": true }
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

Here is a footnote reference[^1] and inline math $e^{i\pi} + 1 = 0$, with $\alpha^2 + \beta_i$ and $\frac{a}{b}$ in running text,
typeset with the OpenType MATH table of Cambria Math: $\sqrt{x^2 + y^2} \le \|v\|$.

$$
\int_0^1 x^2 \, dx = \frac{1}{3}
$$

$$
\sum_{k=1}^{n} k = \frac{n(n+1)}{2} \qquad \lim_{x \to 0} \frac{\sin x}{x} = 1
$$

$$
f(x) = \begin{cases} x^2 & x \ge 0 \\ -x & \text{otherwise} \end{cases}
\qquad
A = \begin{pmatrix} a_{11} & a_{12} \\ a_{21} & a_{22} \end{pmatrix}
$$

$$
\left( \frac{\partial \mathcal{L}}{\partial \hat{\theta}} \right)^2 + \sqrt[3]{\mathbb{R}^n} \cdot \vec{v}
$$

$$
\begin{aligned}
f(x) &= (x+1)^2 & g(x) &= x - 1 \\
     &= x^2 + 2x + 1 & &= -(1 - x)
\end{aligned}
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
