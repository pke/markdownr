# External file-change detection + reload offer

**Status:** approved design, not yet implemented
**Date:** 2026-07-13

## Goal

When the file currently shown in the viewer is modified by an external app,
detect it when Markdownr regains focus (and once after launch-restore) and show
a non-blocking banner offering to reload. Reloading re-reads the source and
restores the approximate scroll position.

## Core rule

**Baseline = the modification time of the content snapshot being displayed.**
Offer reload when the source's modification time is newer than the baseline.

- Fresh read (deep link, picker, reload): baseline = source mtime, captured in
  `openFile()`.
- Launch-restore from recents: baseline = the recents cache copy's mtime
  (`Paths.cache/recent-files/<id>.md`) — "when we last read it" — so edits made
  while the app was quit are detected on launch.

## Source descriptors

Detection operates on a descriptor, not a bare URI, so open paths can upgrade
without touching detection/UI code:

```ts
type FileSource = {
  uri: string;
  kind: 'file' | 'content' | 'bookmark';
};
```

| kind | stat strategy | read strategy | used by |
|---|---|---|---|
| `file` | `File(uri).modificationTime` | `File(uri).text()` | deep links, in-place files, folder-cache files |
| `content` | native `lastModified(uri)` (Android `DocumentFile`; `0` → unknown) | native `readFile(uri)` | Android deep links, Android picked files (phase 2) |
| `bookmark` | resolve security-scoped bookmark → stat | resolve → start/stop access → read | iOS picked files (phase 2) |

## Components

### 1. `fileChangeDetection.ts` (new, pure logic, DI'd like `openDeepLink`)

- `checkForExternalChange(source, baselineMtime, statFn) →
  'changed' | 'unchanged' | 'unknown'`
  - null source/baseline, stat failure, or unsupported kind → `'unknown'`
  - `'unknown'` NEVER shows the banner (covers expired scopes, deleted files,
    providers reporting 0)
- `shouldShowBanner(status, sourceMtime, dismissedMtime)` — dismissing stores
  the offered mtime; the same change never re-nags on later focus events, a
  newer edit re-offers.

### 2. `modules/file-picker/` (new native module, phase 2; mirrors folder-picker)

- **iOS (Swift):** `pickFile()` → `UIDocumentPickerViewController(asCopy:
  false)` → `startAccessingSecurityScopedResource` → persist security-scoped
  bookmark; `readFile(key)` / `statFile(key)` resolve the bookmark and wrap
  access. Stale bookmarks are refreshed inside resolve. Bookmark resolution
  distinguishes *deleted* from *scope expired*.
- **Android (Kotlin):** `pickFile()` → `ACTION_OPEN_DOCUMENT` +
  `takePersistableUriPermission`; `readFile(uri)`, `lastModified(uri)` via
  `DocumentFile`.
- Expo config plugin registered in `app.config.js` (same pattern as
  folder-picker).

### 3. App.tsx wiring

- `baselineRef` set in `openFile()` (stat via descriptor, try/catch) and after
  the initial recents restore (cache-copy mtime).
- Check triggers — exactly two: `AppState` change to `'active'`, and once after
  the initial restore. No polling.
- `MarkdownContext` gains: `externalChangeDetected: boolean`,
  `reloadCurrentFile(): Promise<void>`, `dismissExternalChange(): void`.

### 4. `FileChangedBanner.tsx` (new, ThemeSuggestion visual pattern)

- Rendered in ViewerScreen alongside `ThemeSuggestion` (~line 1102).
- Theme-aware; strings in `i18n.ts` for **en/de/ru** (proper Umlauts in de):
  message ("File changed on disk"), actions ("Reload", "Dismiss").

### 5. Reload flow

1. Capture current scroll position as a percent (existing `scrollToPercent`
   mechanism used by search).
2. Re-read via the descriptor's read strategy.
3. `openFile(content, name, uri)` — refreshes recents cache and baseline.
4. Re-apply the captured percent after the new content lays out. Approximate by
   design; same-length edits land back in place.

### 6. Open-path integration (phase 2)

- `useFileOpener` switches from `expo-document-picker` to the native module.
- `openFile` accepts the descriptor; `LAST_FILE_URI` storage and recents
  entries carry it. **Backward compatible:** entries without a descriptor
  behave exactly as today (no banner, no errors).
- Deep links stay `file` / `content`. Folder flow untouched.

## Phasing

- **Phase 1 (pure JS, ships alone):** detection core + banner + reload + scroll
  restore. Live immediately for deep-link/in-place files and recents restores.
  Picker files remain snapshots until phase 2 upgrades their descriptor kind —
  zero rework in detection/UI.
- **Phase 2 (native):** file-picker module + integration; picked files (and
  their recents) become watchable on both platforms.

## Non-goals

- Folders stay copy-based (user decision); folder files never show the banner.
  Folder cache is still refreshed on app launch as today.
- No deleted-file banner for `file`/`content` kinds (indistinguishable from
  permission loss there; `bookmark` kind may surface it later).
- No polling — iPad split view won't trigger detection; revisit if it bites.
- No exact-position scroll restore (percent-based only).
- No `content://` mtime on iOS (not a thing) and no reliance on providers that
  report 0 (treated as unknown).

## Error handling

Every stat/read failure degrades to silence: `'unknown'` status, no banner, no
console noise beyond debug logging. Reload read failure keeps the current
content and marks the change dismissed — no retry-nag loop for a read that can
no longer succeed; a *newer* external edit re-offers as usual. Nothing in this
feature can lose user data — the viewer is read-only.

## Testing

- **Vitest** (mirrors `openDeepLink` DI tests): `checkForExternalChange` per
  kind × {newer, equal, older, stat-null, stat-throws, unsupported};
  `shouldShowBanner` dismiss/re-offer matrix; reload chain with injected
  reader (success, failure keeps content).
- **Native (phase 2):** manual verification on iPhone simulator + Pixel 8;
  existing deep-link XCUITest keeps guarding the in-place open path.
- **Maestro:** not applicable — external edits can't be driven from Maestro.
  Manual sim check: edit the file in the app's Documents via shell, refocus.
