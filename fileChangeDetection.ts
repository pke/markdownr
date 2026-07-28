import {File} from 'expo-file-system/next';
import {readPickedFile, statPickedFile} from 'file-picker';

/**
 * How the currently displayed file can be re-read/re-statted:
 * - 'file': plain file:// path (deep links, in-place Documents files)
 * - 'content': Android SAF content:// uri (picked/deep-linked; the uri is the ref)
 * - 'bookmark': iOS security-scoped bookmark from the native file-picker
 *   (`ref` holds the bookmark key)
 * See docs/superpowers/specs/2026-07-13-external-file-change-reload-design.md.
 */
export type FileSourceKind = 'file' | 'content' | 'bookmark';

export type FileSource = {
  uri: string;
  kind: FileSourceKind;
  /** Read/stat handle for picked kinds (iOS bookmarkKey; Android uses the uri). */
  ref?: string;
};

export type ChangeStatus = 'changed' | 'unchanged' | 'unknown';

export type ChangeCheck = {
  status: ChangeStatus;
  /** The source's modification time when status is 'changed'/'unchanged'. */
  sourceMtime: number | null;
};

/**
 * Default stat strategy. Returns null (→ 'unknown', no banner) whenever the
 * mtime cannot be trusted: unsupported kind, non-file scheme, missing file,
 * or a provider reporting 0.
 */
export async function statFileSource(source: FileSource): Promise<number | null> {
  if (source.kind === 'bookmark' || source.kind === 'content') {
    const ref = source.ref ?? (source.kind === 'content' ? source.uri : null);
    if (!ref) return null;
    const mtime = await statPickedFile(ref);
    return mtime && mtime > 0 ? mtime : null;
  }
  if (!source.uri.startsWith('file://')) return null;
  const mtime = new File(source.uri).modificationTime;
  return mtime && mtime > 0 ? mtime : null;
}

/** Default read strategy (mirrors statFileSource's kind support). */
export async function readFileSource(source: FileSource): Promise<string> {
  if (source.kind === 'bookmark' || source.kind === 'content') {
    const ref = source.ref ?? (source.kind === 'content' ? source.uri : null);
    if (!ref) throw new Error(`no ref for ${source.kind} source`);
    return readPickedFile(ref);
  }
  return new File(source.uri).text();
}

/**
 * Compare the source's current mtime against the displayed snapshot's
 * baseline. Every failure degrades to 'unknown' — the caller shows nothing.
 */
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

/**
 * Dismissing remembers the offered mtime: the same change never re-nags on
 * later focus events; only a strictly newer edit re-offers.
 */
export function shouldShowBanner(check: ChangeCheck, dismissedMtime: number | null): boolean {
  if (check.status !== 'changed' || check.sourceMtime == null) return false;
  return dismissedMtime == null || check.sourceMtime > dismissedMtime;
}

/**
 * Re-read the source and hand it back through openFile (which refreshes the
 * recents cache and baseline). Returns false when the read fails — the caller
 * keeps the current content and marks the change dismissed.
 */
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
