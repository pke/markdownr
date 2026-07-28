# External File-Change Detection + Reload (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect external modification of the currently displayed file when the app regains focus (and once after launch-restore) and show a banner offering reload with approximate scroll restore.

**Architecture:** A pure, DI'd detection module (`fileChangeDetection.ts`, same pattern as `openDeepLink`) + refs/state wired in `App.tsx` (where `openFile`/`currentFileUri` live) + a `FileChangedBanner` styled after `ThemeSuggestion`. Baseline = mtime of the displayed snapshot; recents restores use the cache copy's mtime. Spec: `docs/superpowers/specs/2026-07-13-external-file-change-reload-design.md` (Phase 1 only — the native `file-picker` module is Phase 2, a separate plan).

**Tech Stack:** React Native / Expo SDK 54, `expo-file-system/next` File API, RN `AppState`, react-i18next (en/de/ru), Vitest.

## Global Constraints

- **Never commit automatically.** The user commits on explicit request only — no `git commit` steps in this plan; report when work is ready to commit.
- All user-facing strings in **en, de, ru** (`i18n.ts`); German with proper Umlauts (ä ö ü ß).
- Banner colors come from `theme.colors` / `backgroundColor` (same as ThemeSuggestion) — keeps WCAG AA compliance inherited from themes.
- `'unknown'` detection status NEVER shows the banner (silent no-op on any stat/read failure).
- MMKV v4 via existing `settings.ts` helpers only; tests run under Vitest with the aliased mocks in `__tests__/mocks/`.
- TypeScript must stay clean: `npx tsc --noEmit` after each task.

---

### Task 1: Detection core — `fileChangeDetection.ts`

**Files:**
- Create: `fileChangeDetection.ts`
- Modify: `__tests__/mocks/expo-file-system-next.ts` (add `modificationTime` support)
- Test: `__tests__/fileChangeDetection.test.ts`

**Interfaces:**
- Consumes: `File` from `expo-file-system/next` (mocked in tests).
- Produces (used by Tasks 4–5):
  - `type FileSourceKind = 'file' | 'content' | 'bookmark'`
  - `type FileSource = {uri: string; kind: FileSourceKind}`
  - `type ChangeCheck = {status: 'changed' | 'unchanged' | 'unknown'; sourceMtime: number | null}`
  - `statFileSource(source: FileSource): Promise<number | null>`
  - `readFileSource(source: FileSource): Promise<string>`
  - `checkForExternalChange(source: FileSource | null, baselineMtime: number | null, statFn?): Promise<ChangeCheck>`
  - `shouldShowBanner(check: ChangeCheck, dismissedMtime: number | null): boolean`
  - `reloadFromSource(source: FileSource, fileName: string | null, openFile: (content: string, name: string | null, uri: string) => void, readFn?): Promise<boolean>`

- [ ] **Step 1: Extend the FS mock with modification times**

In `__tests__/mocks/expo-file-system-next.ts`, add an mtime store next to `fileStore` and expose it on `File` plus a test helper:

```ts
const mtimeStore = new Map<string, number>();

// inside class File add:
  get modificationTime(): number | null {
    return mtimeStore.get(this.uri) ?? null;
  }

// at the bottom, add test helpers:
export function __setMtime(uri: string, mtime: number): void {
  mtimeStore.set(uri, mtime);
}
export function __clearMtimes(): void {
  mtimeStore.clear();
}
```

- [ ] **Step 2: Write the failing tests**

`__tests__/fileChangeDetection.test.ts`:

```ts
import {describe, it, expect, vi} from 'vitest';
import {
  checkForExternalChange,
  shouldShowBanner,
  statFileSource,
  reloadFromSource,
  type FileSource,
} from '../fileChangeDetection';
import {__setMtime, __clearMtimes} from './mocks/expo-file-system-next';

const src = (uri = 'file:///docs/a.md'): FileSource => ({uri, kind: 'file'});
const statOf = (v: number | null) => vi.fn().mockResolvedValue(v);

describe('checkForExternalChange', () => {
  it('is unknown for null source or null baseline', async () => {
    expect((await checkForExternalChange(null, 100, statOf(200))).status).toBe('unknown');
    expect((await checkForExternalChange(src(), null, statOf(200))).status).toBe('unknown');
  });

  it('is changed when source mtime is newer than baseline', async () => {
    const check = await checkForExternalChange(src(), 100, statOf(200));
    expect(check).toEqual({status: 'changed', sourceMtime: 200});
  });

  it('is unchanged for equal or older mtimes', async () => {
    expect((await checkForExternalChange(src(), 200, statOf(200))).status).toBe('unchanged');
    expect((await checkForExternalChange(src(), 200, statOf(100))).status).toBe('unchanged');
  });

  it('is unknown when stat returns null, 0, or throws', async () => {
    expect((await checkForExternalChange(src(), 100, statOf(null))).status).toBe('unknown');
    expect((await checkForExternalChange(src(), 100, statOf(0))).status).toBe('unknown');
    const throwing = vi.fn().mockRejectedValue(new Error('gone'));
    expect((await checkForExternalChange(src(), 100, throwing)).status).toBe('unknown');
  });
});

describe('statFileSource (default stat strategy)', () => {
  it('reads modificationTime for file:// sources', async () => {
    __clearMtimes();
    __setMtime('file:///docs/a.md', 1234);
    expect(await statFileSource(src())).toBe(1234);
  });

  it('returns null for non-file kinds (phase 2 strategies)', async () => {
    expect(await statFileSource({uri: 'content://x/1', kind: 'content'})).toBeNull();
    expect(await statFileSource({uri: 'file:///x', kind: 'bookmark'})).toBeNull();
  });

  it('returns null for a file kind whose uri is not file://', async () => {
    expect(await statFileSource({uri: 'content://x/1', kind: 'file'})).toBeNull();
  });

  it('returns null when the file has no mtime (missing/unreadable)', async () => {
    __clearMtimes();
    expect(await statFileSource(src('file:///nope.md'))).toBeNull();
  });
});

describe('shouldShowBanner', () => {
  const changed = {status: 'changed' as const, sourceMtime: 300};

  it('shows for a changed status with no prior dismissal', () => {
    expect(shouldShowBanner(changed, null)).toBe(true);
  });

  it('never shows for unchanged or unknown', () => {
    expect(shouldShowBanner({status: 'unchanged', sourceMtime: 300}, null)).toBe(false);
    expect(shouldShowBanner({status: 'unknown', sourceMtime: null}, null)).toBe(false);
  });

  it('stays hidden for the same dismissed change, re-offers for a newer one', () => {
    expect(shouldShowBanner(changed, 300)).toBe(false); // same mtime dismissed
    expect(shouldShowBanner(changed, 400)).toBe(false); // dismissed even newer
    expect(shouldShowBanner({status: 'changed', sourceMtime: 500}, 300)).toBe(true);
  });
});

describe('reloadFromSource', () => {
  it('reads and re-opens, keeping the current file name', async () => {
    const read = vi.fn().mockResolvedValue('# New');
    const openFile = vi.fn();
    const ok = await reloadFromSource(src(), 'a.md', openFile, read);
    expect(ok).toBe(true);
    expect(read).toHaveBeenCalledWith(src());
    expect(openFile).toHaveBeenCalledWith('# New', 'a.md', 'file:///docs/a.md');
  });

  it('returns false and does not open when the read fails', async () => {
    const read = vi.fn().mockRejectedValue(new Error('scope expired'));
    const openFile = vi.fn();
    expect(await reloadFromSource(src(), 'a.md', openFile, read)).toBe(false);
    expect(openFile).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `npx vitest run __tests__/fileChangeDetection.test.ts`
Expected: FAIL — cannot resolve `../fileChangeDetection`.

- [ ] **Step 4: Implement `fileChangeDetection.ts`**

```ts
import {File} from 'expo-file-system/next';

/** How the currently displayed file can be re-read/re-statted. Phase 1 only
 * implements 'file'; 'content' (Android SAF) and 'bookmark' (iOS security-
 * scoped) get native strategies in Phase 2. */
export type FileSourceKind = 'file' | 'content' | 'bookmark';

export type FileSource = {
  uri: string;
  kind: FileSourceKind;
};

export type ChangeStatus = 'changed' | 'unchanged' | 'unknown';

export type ChangeCheck = {
  status: ChangeStatus;
  /** The source's modification time when status is 'changed'/'unchanged'. */
  sourceMtime: number | null;
};

/** Default stat strategy. Returns null (→ 'unknown', no banner) whenever the
 * mtime cannot be trusted: unsupported kind, non-file scheme, missing file,
 * or a provider reporting 0. */
export async function statFileSource(source: FileSource): Promise<number | null> {
  if (source.kind !== 'file' || !source.uri.startsWith('file://')) return null;
  const mtime = new File(source.uri).modificationTime;
  return mtime && mtime > 0 ? mtime : null;
}

/** Default read strategy (mirrors statFileSource's kind support). */
export async function readFileSource(source: FileSource): Promise<string> {
  if (source.kind !== 'file') {
    throw new Error(`unsupported source kind: ${source.kind}`);
  }
  return new File(source.uri).text();
}

/** Compare the source's current mtime against the displayed snapshot's
 * baseline. Every failure degrades to 'unknown' — the caller shows nothing. */
export async function checkForExternalChange(
  source: FileSource | null,
  baselineMtime: number | null,
  statFn: (s: FileSource) => Promise<number | null> = statFileSource,
): Promise<ChangeCheck> {
  if (!source || baselineMtime == null) return {status: 'unknown', sourceMtime: null};
  let mtime: number | null;
  try {
    mtime = await statFn(source);
  } catch {
    mtime = null;
  }
  if (!mtime || mtime <= 0) return {status: 'unknown', sourceMtime: null};
  return mtime > baselineMtime
    ? {status: 'changed', sourceMtime: mtime}
    : {status: 'unchanged', sourceMtime: mtime};
}

/** Dismissing remembers the offered mtime: the same change never re-nags on
 * later focus events; only a strictly newer edit re-offers. */
export function shouldShowBanner(check: ChangeCheck, dismissedMtime: number | null): boolean {
  if (check.status !== 'changed' || check.sourceMtime == null) return false;
  return dismissedMtime == null || check.sourceMtime > dismissedMtime;
}

/** Re-read the source and hand it back through openFile (which refreshes the
 * recents cache and baseline). Returns false when the read fails — the caller
 * keeps the current content and marks the change dismissed. */
export async function reloadFromSource(
  source: FileSource,
  fileName: string | null,
  openFile: (content: string, name: string | null, uri: string) => void,
  readFn: (s: FileSource) => Promise<string> = readFileSource,
): Promise<boolean> {
  let content: string;
  try {
    content = await readFn(source);
  } catch {
    return false;
  }
  openFile(content, fileName, source.uri);
  return true;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npx vitest run __tests__/fileChangeDetection.test.ts`
Expected: PASS (15 tests). Then `npx vitest run` — full suite green (285 existing + 15). Then `npx tsc --noEmit` — clean.

---

### Task 2: i18n strings (en/de/ru)

**Files:**
- Modify: `i18n.ts` (three locale objects)

**Interfaces:**
- Produces: `t('fileChanged.message')`, `t('fileChanged.reload')` for Task 5. Dismiss reuses existing `t('common.dismiss')`.

- [ ] **Step 1: Add the keys**

In `i18n.ts`, add to the `en` object (after the `error` key, before `common`):

```ts
  fileChanged: {message: 'File changed on disk', reload: 'Reload'},
```

To `de` (same position — the `Translations<typeof en>` type forces key parity):

```ts
  fileChanged: {message: 'Datei wurde extern geändert', reload: 'Neu laden'},
```

To `ru`:

```ts
  fileChanged: {message: 'Файл изменён на диске', reload: 'Перезагрузить'},
```

- [ ] **Step 2: Verify via typecheck + suite**

Run: `npx tsc --noEmit` — clean (the `Translations<typeof en>` mapped type fails compilation if any locale misses the key — that's the parity test).
Run: `npx vitest run __tests__/i18n.test.ts` — existing 7 tests still PASS.

---

### Task 3: `getCachedFileMtime` in recentFiles.ts

**Files:**
- Modify: `recentFiles.ts`
- Test: `__tests__/recentFiles-mtime.test.ts` (new)

**Interfaces:**
- Consumes: existing `recentFilesDir` module-level constant, mock `__setMtime`.
- Produces: `getCachedFileMtime(id: string): number | null` — used by Task 4 as the baseline for launch-restores.

- [ ] **Step 1: Write the failing test**

`__tests__/recentFiles-mtime.test.ts`:

```ts
import {describe, it, expect} from 'vitest';
import {getCachedFileMtime} from '../recentFiles';
import {__setMtime, __clearMtimes} from './mocks/expo-file-system-next';

describe('getCachedFileMtime', () => {
  it('returns the cache copy mtime for an entry id', () => {
    __clearMtimes();
    // Mock Paths.cache is '/mock-cache'; Directory/File join parts with '/'.
    __setMtime('/mock-cache/recent-files/abc123.md', 777);
    expect(getCachedFileMtime('abc123')).toBe(777);
  });

  it('returns null for a missing cache file', () => {
    __clearMtimes();
    expect(getCachedFileMtime('nope')).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run __tests__/recentFiles-mtime.test.ts`
Expected: FAIL — `getCachedFileMtime` is not exported.

- [ ] **Step 3: Implement**

In `recentFiles.ts`, after `loadRecentFile`:

```ts
/** Modification time of a recents cache copy — the "when we last read it"
 * baseline for external-change detection after a launch-restore. */
export function getCachedFileMtime(id: string): number | null {
  try {
    const file = new File(recentFilesDir, `${id}.md`);
    if (!file.exists) return null;
    const mtime = file.modificationTime;
    return mtime && mtime > 0 ? mtime : null;
  } catch {
    return null;
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run` — full suite green. `npx tsc --noEmit` — clean.

---

### Task 4: Context extension + App.tsx wiring

**Files:**
- Modify: `MarkdownContext.tsx` (type + defaults)
- Modify: `App.tsx`

**Interfaces:**
- Consumes: Task 1 (`checkForExternalChange`, `shouldShowBanner`, `statFileSource`, `readFileSource`, `reloadFromSource`, `FileSource`), Task 3 (`getCachedFileMtime`).
- Produces (context, used by Task 5): `externalChangeDetected: boolean`, `reloadCurrentFile(): Promise<void>`, `dismissExternalChange(): void`.

- [ ] **Step 1: Extend the context type and defaults**

`MarkdownContext.tsx` — add to `MarkdownContextType`:

```ts
  externalChangeDetected: boolean;
  reloadCurrentFile: () => Promise<void>;
  dismissExternalChange: () => void;
```

and to the `createContext` defaults:

```ts
  externalChangeDetected: false,
  reloadCurrentFile: async () => {},
  dismissExternalChange: () => {},
```

- [ ] **Step 2: Wire detection state in App.tsx**

Imports to add:

```ts
import {AppState} from 'react-native'; // extend the existing react-native import
import {
  checkForExternalChange, shouldShowBanner, statFileSource, readFileSource,
  reloadFromSource, type FileSource,
} from './fileChangeDetection';
import {getCachedFileMtime} from './recentFiles'; // extend existing import
```

State/refs (place after `openedViaDeepLink`):

```ts
  const currentSourceRef = useRef<FileSource | null>(null);
  const baselineMtimeRef = useRef<number | null>(null);
  const dismissedMtimeRef = useRef<number | null>(null);
  const pendingMtimeRef = useRef<number | null>(null);
  const [externalChangeDetected, setExternalChangeDetected] = useState(false);

  const sourceForUri = (uri: string | null): FileSource | null =>
    uri?.startsWith('file://') ? {uri, kind: 'file'}
    : uri?.startsWith('content://') ? {uri, kind: 'content'}
    : null;
```

Extend `openFile` (inside the existing useCallback, after `Storage.setString(...)`):

```ts
    const source = sourceForUri(uri);
    currentSourceRef.current = source;
    baselineMtimeRef.current = null;
    dismissedMtimeRef.current = null;
    pendingMtimeRef.current = null;
    setExternalChangeDetected(false);
    if (source) {
      // Fire-and-forget: mtime of the file we just read is the new baseline.
      statFileSource(source).then(m => { baselineMtimeRef.current = m; }).catch(() => {});
    }
```

Check trigger (new callback + effect):

```ts
  const runExternalChangeCheck = useCallback(async () => {
    const check = await checkForExternalChange(currentSourceRef.current, baselineMtimeRef.current);
    if (shouldShowBanner(check, dismissedMtimeRef.current)) {
      pendingMtimeRef.current = check.sourceMtime;
      setExternalChangeDetected(true);
    }
  }, []);

  useEffect(() => {
    const sub = AppState.addEventListener('change', (state) => {
      if (state === 'active') runExternalChangeCheck();
    });
    return () => sub.remove();
  }, [runExternalChangeCheck]);
```

Launch-restore baseline — in the existing `Linking.getInitialURL()` effect, inside the `loadRecentFile(initialRecent).then((content) => {...})` success branch (where `setMarkdownContent(content)` runs):

```ts
              currentSourceRef.current = sourceForUri(Storage.getString(StorageKeys.LAST_FILE_URI) || null);
              baselineMtimeRef.current = getCachedFileMtime(initialRecent.id);
              runExternalChangeCheck();
```

Reload/dismiss actions:

```ts
  const dismissExternalChange = useCallback(() => {
    dismissedMtimeRef.current = pendingMtimeRef.current;
    setExternalChangeDetected(false);
  }, []);

  const reloadCurrentFile = useCallback(async () => {
    const source = currentSourceRef.current;
    if (!source) { setExternalChangeDetected(false); return; }
    const ok = await reloadFromSource(source, fileName, openFile, readFileSource);
    if (!ok) dismissExternalChange(); // read failed: keep content, stop nagging
  }, [fileName, openFile, dismissExternalChange]);
```

(`reloadFromSource` → `openFile` already resets the banner state and re-baselines.)

Add all three to `contextValue` and its dependency array: `externalChangeDetected, reloadCurrentFile, dismissExternalChange`.

- [ ] **Step 3: Verify**

Run: `npx tsc --noEmit` — clean. `npx vitest run` — full suite green (the logic itself was tested in Tasks 1/3; this task is wiring).

---

### Task 5: `FileChangedBanner.tsx` + ViewerScreen integration

**Files:**
- Create: `FileChangedBanner.tsx`
- Modify: `ViewerScreen.tsx` (~line 1102, next to `ThemeSuggestion`)

**Interfaces:**
- Consumes: context from Task 4, i18n keys from Task 2, `setScrollToPercent` (existing context), ViewerScreen's `lastScrollY` ref + `contentHeight` state (existing, ~lines 636–637).

- [ ] **Step 1: Create the banner component**

`FileChangedBanner.tsx` (visual pattern copied from ThemeSuggestion's banner variant):

```tsx
import React, {useCallback, useContext} from 'react';
import {StyleSheet, Text, TouchableOpacity, View} from 'react-native';
import Animated, {FadeInUp, FadeOutDown} from 'react-native-reanimated';
import {useTranslation} from 'react-i18next';

import {MarkdownContext} from './MarkdownContext';

type FileChangedBannerProps = {
  bottomInset?: number;
  /** Current scroll position as a fraction of content height, for restore. */
  getScrollPercent?: () => number | null;
};

export function FileChangedBanner({bottomInset, getScrollPercent}: FileChangedBannerProps) {
  const {t} = useTranslation();
  const {
    externalChangeDetected,
    reloadCurrentFile,
    dismissExternalChange,
    theme,
    backgroundColor,
    setScrollToPercent,
  } = useContext(MarkdownContext);

  const handleReload = useCallback(async () => {
    const percent = getScrollPercent?.() ?? null;
    await reloadCurrentFile();
    if (percent !== null) setScrollToPercent(percent);
  }, [getScrollPercent, reloadCurrentFile, setScrollToPercent]);

  if (!externalChangeDetected) return null;

  return (
    <Animated.View
      entering={FadeInUp.duration(300)}
      exiting={FadeOutDown.duration(200)}
      testID="fileChangedBanner"
      style={[
        styles.container,
        {bottom: bottomInset, right: 20, backgroundColor, borderColor: theme.colors.border},
      ]}>
      <Text style={[styles.text, {color: theme.colors.text}]}>
        {t('fileChanged.message')}
      </Text>
      <View style={styles.actions}>
        <TouchableOpacity
          onPress={dismissExternalChange}
          style={[styles.dismissButton, {borderColor: theme.colors.border}]}>
          <Text style={[styles.dismissText, {color: theme.colors.text}]}>{t('common.dismiss')}</Text>
        </TouchableOpacity>
        <TouchableOpacity
          onPress={handleReload}
          testID="fileChangedReloadButton"
          style={[styles.button, {backgroundColor: theme.colors.link}]}>
          <Text style={styles.buttonText}>{t('fileChanged.reload')}</Text>
        </TouchableOpacity>
      </View>
    </Animated.View>
  );
}

const styles = StyleSheet.create({
  container: {
    position: 'absolute',
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 14,
    paddingVertical: 10,
    borderRadius: 16,
    borderWidth: 1,
    shadowColor: '#000',
    shadowOffset: {width: 0, height: 2},
    shadowOpacity: 0.15,
    shadowRadius: 8,
    elevation: 4,
    gap: 10,
  },
  text: {fontSize: 14, fontWeight: '500'},
  actions: {flexDirection: 'row', alignItems: 'center', gap: 8},
  button: {paddingHorizontal: 14, paddingVertical: 6, borderRadius: 8},
  buttonText: {color: '#ffffff', fontSize: 13, fontWeight: '600'},
  dismissButton: {paddingHorizontal: 8, paddingVertical: 6},
  dismissText: {fontSize: 13},
});
```

- [ ] **Step 2: Render it in ViewerScreen**

Import: `import {FileChangedBanner} from './FileChangedBanner';`

Next to the existing `<ThemeSuggestion variant="banner" bottomInset={insets.bottom + 56 + 12} />` (~line 1102), add — stacked one banner-height higher so both can show:

```tsx
        <FileChangedBanner
          bottomInset={insets.bottom + 56 + 12 + 52}
          getScrollPercent={() =>
            contentHeight > 0 ? lastScrollY.current / contentHeight : null
          }
        />
```

(`lastScrollY` and `contentHeight` already exist in ViewerScreen at ~lines 636–637.)

- [ ] **Step 3: Verify**

Run: `npx tsc --noEmit` — clean. `npx vitest run` — full suite green.

---

### Task 6: End-to-end verification on the simulator

**Files:** none (verification only).

Prereqs: Metro running on port 8092 (`APP_VARIANT=development npx expo start --port 8092`), dev app installed on the iPhone 17 Pro simulator (`C77D6539-FB9A-48FD-B287-43DEB25C324A`) — both already true from the XCUITest work; Metro hot-reloads the JS changes, no rebuild needed.

- [ ] **Step 1: Seed a watchable file and open it via deep link**

```bash
SIM=C77D6539-FB9A-48FD-B287-43DEB25C324A
APP=dev.dudesoft.markdownr.dev
C=$(xcrun simctl get_app_container "$SIM" "$APP" data)
printf '# Watch Test\n\noriginal content v1\n' > "$C/Documents/watch-test.md"
xcrun simctl openurl "$SIM" "file://$C/Documents/watch-test.md"
```

Screenshot: expect "Watch Test / original content v1" rendered, **no banner**.

- [ ] **Step 2: Background the app, edit externally, refocus**

```bash
xcrun simctl launch "$SIM" com.apple.Preferences   # backgrounds Markdownr
sleep 2
printf '# Watch Test\n\nEDITED EXTERNALLY v2\n' > "$C/Documents/watch-test.md"
xcrun simctl launch "$SIM" "$APP"                   # refocus → AppState 'active'
sleep 3
```

Screenshot: expect the **"File changed on disk"** banner. Content still shows v1.

- [ ] **Step 3: Tap Reload via an ad-hoc Maestro flow**

```yaml
# scratchpad/reload-tap.yaml
appId: dev.dudesoft.markdownr.dev
---
- launchApp:
    clearState: false
- tapOn: 'Reload'
```

Run: `maestro --device "$SIM" test scratchpad/reload-tap.yaml`
Screenshot: content now shows "EDITED EXTERNALLY v2", banner gone.

- [ ] **Step 4: Dismiss path + no-renag**

Repeat Step 2 (edit v3, refocus) but tap **Dismiss** instead. Then background/refocus once more **without** editing: banner must NOT reappear (same mtime dismissed). Edit v4 + refocus: banner reappears.

- [ ] **Step 5: Full suite + report**

Run: `npx vitest run` and `npx tsc --noEmit`. Report results to the user and state that the work is ready to commit — **do not commit**.
