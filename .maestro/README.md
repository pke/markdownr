# Maestro E2E tests

End-to-end UI flows for Markdownr, driven by [Maestro](https://maestro.mobile.dev).
These complement the Vitest unit tests (`__tests__/`, pure logic) by exercising
the app on a real simulator/emulator: launch, rendering, navigation, the FAB
menu, and the landscape regression we just fixed.

This folder already contained the screenshot-automation flows
(`screenshots-*.yaml`, `ipad-screenshots-*.yaml`, run via `fastlane upload_screenshots`).
The `01`–`05` flows are the new **test** flows and follow the same conventions:
`appId: dev.dudesoft.markdownr.dev` (the dev variant) and liberal
`waitForAnimationToEnd` between steps.

## Install Maestro

```sh
curl -fsSL "https://get.maestro.mobile.dev" | bash
# then restart your shell, or: export PATH="$PATH:$HOME/.maestro/bin"
maestro --version
```

## Prerequisites

- The **dev build installed and running** on a booted simulator, with Metro up
  (the debug build loads JS from Metro on port 8092):
  ```sh
  APP_VARIANT=development npx expo run:ios          # builds, installs, starts Metro
  ```
  The flows target `dev.dudesoft.markdownr.dev`, so use the dev build (same as
  the screenshot flows). See project memory `simulator-deploy.md` for the
  bundle-id gotcha if you build with raw `xcodebuild`.

## Run

```sh
# just the test flows
maestro test .maestro/01-launch-and-render.yaml
maestro test .maestro/02-toc-drawer.yaml
# ...or a whole run (this also picks up the screenshot flows):
maestro test .maestro/

# interactive selector explorer (great while writing flows)
maestro studio
```

Screenshots (`takeScreenshot`) land in the working directory.

## Flows

| Flow | What it covers |
|------|----------------|
| `01-launch-and-render` | App launches; welcome markdown renders |
| `02-toc-drawer` | Table-of-Contents drawer opens with auto-generated headings |
| `03-fab-menu` | FAB opens, menu items reachable, item tap works (guards the FAB hit-area fix) |
| `04-load-sample` | Loads a different document via an in-app link; re-renders |
| `05-landscape-menu` | Landscape regression: menu items stay on-screen (needs `setOrientation` support) |
| `06-frontmatter-theme` | `theme:` front matter shows the suggestion banner; Apply activates the theme |
| `07-drawer-landscape` | Drawer opens & renders in landscape-left (guards the left safe-area inset fix) |

The flows lean on the app's built-in content (welcome page, Ocean sample) and
`testID`s, so they need **no external fixtures** and **never touch the native
file picker** — which is deliberately hard to automate.

## Assumptions & tips

- The device locale is **English** (assertions use English strings). For a
  localization flow, launch with a locale override and assert translated text —
  e.g. `xcrun simctl launch <UDID> dev.dudesoft.markdownr.dev -AppleLanguages "(ru)"`
  then assert `"Добро пожаловать в Markdownr"`.
- Text selectors are **full-match regex**, not substring. To match part of an
  element's text (e.g. a label with an emoji like `Suggested theme: 🌊 Ocean`),
  wildcard it: `"Suggested theme.*Ocean"`. Opening the drawer uses an edge
  `swipe` rather than tapping the `☰` link for the same reason.
- `assertVisible` does not scroll — use `scrollUntilVisible` for content below
  the fold. Elements behind the menu's dimming backdrop count as not visible.

## Next steps

- **Real file loading:** `fixtures/sample.md` is here for testing the actual
  file path. Push it onto the simulator and open it via a `file://` deep link
  (`openLink:`), which exercises front-matter parsing + relative links end-to-end.
- **Search:** add a flow that opens Search and types a query. The native iOS
  search bar needs `tapOn` the field then `inputText`; verify selectors in
  `maestro studio` first.
- **CI:** run on real devices via [Maestro Cloud](https://maestro.mobile.dev),
  or on a simulator in GitHub Actions with `maestro test`.
