# External File-Change Reload — Phase 2 (native file-picker) Implementation Plan

> Executed inline in the authoring session (context hot); code lives in the
> implementation commits. Spec: `../specs/2026-07-13-external-file-change-reload-design.md`.

**Goal:** Picked files become watchable: iOS via security-scoped bookmarks, Android via persistable `content://` grants. Recents entries carry the source so re-opened files stay live.

## Tasks

### 1. `modules/file-picker/` native module (mirrors folder-picker)
- `package.json`, `expo-module.config.json`, `index.ts`, `plugin.js` (Android source copy), `ios/FilePicker.podspec`, `ios/FilePickerModule.swift`, `android/src/main/java/dev/dudesoft/filepicker/FilePickerModule.kt`.
- JS API: `pickFile(): Promise<PickedFile | null>` (`{uri, name, bookmarkKey?}`),
  `readPickedFile(ref): Promise<string>`, `statPickedFile(ref): Promise<number | null>`.
- iOS: `UIDocumentPickerViewController(forOpeningContentTypes:, asCopy: false)`;
  on pick: start security scope → `bookmarkData()` → persist in UserDefaults
  (`FilePickerBookmarks` dict, `bookmarkKey` = UUID). `readPickedFile`/`statPickedFile`
  resolve the bookmark (refresh when stale), wrap access, read text / contentModificationDate (ms).
- Android: `ACTION_OPEN_DOCUMENT` + `takePersistableUriPermission`; `ref` = the uri.
  `statPickedFile` queries `COLUMN_LAST_MODIFIED` via contentResolver (no extra deps);
  `0` → null.
- Register in root `package.json` (`"file-picker": "file:./modules/file-picker"`) and
  `app.config.js` plugins (like folder-picker). Vitest alias + mock.

### 2. Detection strategies for `bookmark`/`content` kinds
- `FileSource` gains optional `ref` (bookmarkKey on iOS; uri doubles as ref on Android).
- `statFileSource`: `bookmark`/`content` → `statPickedFile(source.ref ?? source.uri)`.
- `readFileSource`: same shape via `readPickedFile`. All failures still degrade to null/'unknown'.
- Unit tests via the file-picker mock.

### 3. Source persistence
- `StorageKeys.LAST_FILE_SOURCE` (JSON descriptor) written in `openFile`; launch-restore
  parses it (fallback: derive from `LAST_FILE_URI`).
- `RecentFileEntry.source?: FileSource`; `addRecentFile(content, name, source?)`.
  Old entries (no source) behave exactly as before.
- `openFile(content, name, uri?, source?)` — optional 4th arg, default derived from uri.

### 4. Open-path integration
- `useFileOpener`: replace `expo-document-picker` with `pickFile()` → `readPickedFile`
  → `openFile(content, name, uri, source)`.
- Recents tap (`RecentFilesScreen`): when the entry has a source, try the live read
  first (fresher than cache; baseline lands correctly), fall back to the cache copy.
- Deep links / folder flow unchanged.

### 5. Verification
- Full vitest suite green; no new tsc errors in touched files.
- iOS: register plugin → `expo prebuild -p ios` (re-apply Metro patch if reset) →
  build → install on iPhone 17 Pro sim → drive the system picker with Maestro
  (pick the fixture from On My iPhone/Markdownr Dev) → edit externally → banner →
  reload. Screenshot evidence at each step.
- Android: `gradlew assembleDebug` compile check (best effort; no device attached).

### 6. Commit
- `feat: make picked files watchable via native file picker` (auto-commit authorized).
