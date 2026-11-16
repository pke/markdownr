import {useCallback, useContext} from 'react';
import {MarkdownContext} from './MarkdownContext';
import {pickFile, readPickedFile} from 'file-picker';
import {pickFolder, type FolderResult} from 'folder-picker';
import {Storage, StorageKeys} from './settings';
import type {FileSource} from './fileChangeDetection';
import {readFolderDocumentOrIndex, sortFiles, type OpenDocument} from './folderNavigation';

export function useFileOpener(onOpen?: () => void) {
  const {openFile} = useContext(MarkdownContext);

  return useCallback(async () => {
    onOpen?.();

    try {
      // Native in-place picker (no cache copy): the picked file stays live, so
      // external edits are detectable (see fileChangeDetection.ts).
      const picked = await pickFile();
      if (!picked) return;

      const source: FileSource = picked.bookmarkKey
        ? {uri: picked.uri, kind: 'bookmark', ref: picked.bookmarkKey}
        : {uri: picked.uri, kind: 'content', ref: picked.uri};
      const content = await readPickedFile(source.ref!);
      openFile(content, picked.name ?? null, picked.uri, source);
    } catch (err) {
      console.error('Error picking document:', err);
    }
  }, [openFile, onOpen]);
}

/**
 * Only a newly picked folder auto-opens a numeric or sole file.
 */
export async function openFolderResult(result: FolderResult, openFile: OpenDocument) {
  const sorted = sortFiles(result.files);
  const first = sorted[0];
  const direct = first && (sorted.length === 1 || /^\d/.test(first.name)) ? first : undefined;
  openFile(...await readFolderDocumentOrIndex(result, direct));
}

export function useFolderOpener(onOpen?: () => void) {
  const {openFile} = useContext(MarkdownContext);

  return useCallback(async () => {
    onOpen?.();

    try {
      const result = await pickFolder();
      if (result) {
        await openFolderResult(result, openFile);
      }
    } catch (err) {
      console.error('Error picking folder:', err);
    }
  }, [openFile, onOpen]);
}

/**
 * Resolve a relative markdown link against the currently open file's URI.
 * Returns the resolved file URI, or null if resolution is not possible.
 */
export function resolveRelativeMarkdownLink(href: string, currentFileUri: string | null): string | null {
  if (!currentFileUri) return null;

  // Reject anchors, protocol-relative URLs, and any link that carries its own
  // scheme (http:, mailto:, markdownr:, file:, content: ...). Only bare
  // relative links are resolved against the current file.
  if (href.startsWith('#') || href.startsWith('//') || /^[a-z][a-z0-9+.-]*:/i.test(href)) {
    return null;
  }

  const lastSlash = currentFileUri.lastIndexOf('/');
  if (lastSlash === -1) return null;
  const dir = currentFileUri.substring(0, lastSlash + 1);

  // Resolve + normalize (collapses ./ and ../). A link we can't parse is a
  // link we won't open — no naive string-concat fallback.
  let resolved: string;
  let root: string;
  try {
    resolved = new URL(href, dir).toString();
    // Containment root: the folder the user actually granted access to (the
    // picked folder copied into cache) when the current file lives inside it,
    // otherwise the current file's own directory.
    const folderRoot = Storage.getString(StorageKeys.LAST_FOLDER_URI);
    const rawRoot = folderRoot && currentFileUri.startsWith(folderRoot) ? folderRoot : dir;
    root = new URL(rawRoot).toString();
  } catch {
    return null;
  }
  if (!root.endsWith('/')) root += '/';

  // Defense in depth: a legitimate relative link never needs percent-encoded
  // dots or slashes. Encoded forms (%2e = '.', %2f = '/') can smuggle ../ past
  // URL normalization yet still be decoded by the native file layer, so reject
  // them outright before the containment check.
  if (/%2e|%2f/i.test(resolved)) return null;

  // Block `../` traversal that escapes the granted folder into the app's other
  // private storage, and allow only local file/content schemes (exact match).
  if (!resolved.startsWith(root)) return null;
  const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(resolved)?.[1].toLowerCase();
  if (scheme !== 'file' && scheme !== 'content') return null;

  return resolved;
}
