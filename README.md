# Markdownr

A Markdown viewer for every platform. Website: https://markdownr.dudesoft.app

| Path | What |
|---|---|
| `apps/mobile` | iOS and Android app (Expo, React Native) |
| `apps/desktop` | Windows, macOS and Linux app (Rust, native text and drawing, no runtime) |
| `www` | The website (static, deployed by Vercel) |
| `hooks` | Git hooks (`post-commit` bumps the mobile app's version on `feat`/`fix`/`perf`/`refactor` commits that touch it) |

Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`type(scope): description`); history stays linear (rebase, fast-forward only).
