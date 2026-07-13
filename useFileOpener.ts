import {useCallback, useContext} from 'react';
import * as DocumentPicker from 'expo-document-picker';
import {File} from 'expo-file-system/next';
import {MarkdownContext} from './MarkdownContext';
import {pickFolder, restoreFolder, type FolderFile, type FolderResult} from 'folder-picker';
import {Storage, StorageKeys} from './settings';

export function useFileOpener(onOpen?: () => void) {
  const {openFile} = useContext(MarkdownContext);

  return useCallback(async () => {
    onOpen?.();

    try {
      const result = await DocumentPicker.getDocumentAsync({
        type: ['text/markdown', 'text/plain', 'net.daringfireball.markdown', 'public.plain-text'],
        copyToCacheDirectory: true,
      });

      if (!result.canceled && result.assets && result.assets.length > 0) {
        const file = result.assets[0];
        const content = await new File(file.uri).text();
        openFile(content, file.name ?? null, file.uri);
      }
    } catch (err) {
      console.error('Error picking document:', err);
    }
  }, [openFile, onOpen]);
}

/**
 * Sort files by natural order: numeric prefix first (00, 01, 02...), then by relativePath.
 */
export function sortFiles(files: FolderFile[]): FolderFile[] {
  return [...files].sort((a, b) => {
    const nameA = a.name.toLowerCase();
    const nameB = b.name.toLowerCase();
    const numA = parseInt(nameA.match(/^(\d+)/)?.[1] ?? '', 10);
    const numB = parseInt(nameB.match(/^(\d+)/)?.[1] ?? '', 10);
    if (!isNaN(numA) && !isNaN(numB)) return numA - numB;
    if (!isNaN(numA)) return -1;
    if (!isNaN(numB)) return 1;
    return a.relativePath.localeCompare(b.relativePath);
  });
}

/**
 * Build a markdown index page linking to all files in the folder.
 */
export function buildFolderIndex(files: FolderFile[], folderName: string, baseUri: string): {content: string; uri: string} {
  const lines = [`# ${folderName}`, ''];
  for (const file of files) {
    const label = file.name.replace(/\.(md|markdown)$/i, '');
    // Use relativePath so links resolve correctly from the base URI
    lines.push(`- [${label}](${file.relativePath})`);
  }
  return {content: lines.join('\n'), uri: baseUri};
}

/**
 * Open a folder result: auto-open first numerically named file, or show index.
 */
function openFolderResult(result: FolderResult, openFile: (content: string, name: string | null, uri: string) => void) {
  const sorted = sortFiles(result.files);
  if (sorted.length === 0) return;

  // Persist folder URI for restore on next launch
  Storage.setString(StorageKeys.LAST_FOLDER_URI, result.folderUri);

  const firstName = sorted[0].name.toLowerCase();
  const hasNumericStart = /^\d+/.test(firstName);

  if (hasNumericStart || sorted.length === 1) {
    // Open first file directly; content will be read via File API using the URI
    readAndOpen(sorted[0].uri, sorted[0].name, openFile);
  } else {
    // No obvious entry point — show a file browser as markdown
    const {content, uri} = buildFolderIndex(sorted, result.folderName, sorted[0].uri);
    openFile(content, null, uri);
  }
}

async function readAndOpen(uri: string, name: string, openFile: (content: string, name: string | null, uri: string) => void) {
  try {
    const content = await new File(uri).text();
    openFile(content, name, uri);
  } catch (err) {
    console.error('Error reading file:', uri, err);
  }
}

export function useFolderOpener(onOpen?: () => void) {
  const {openFile} = useContext(MarkdownContext);

  return useCallback(async () => {
    onOpen?.();

    try {
      const result = await pickFolder();
      if (result) {
        openFolderResult(result, openFile);
      }
    } catch (err) {
      console.error('Error picking folder:', err);
    }
  }, [openFile, onOpen]);
}

/**
 * Try to restore the last opened folder on app startup.
 * Returns true if restored successfully.
 */
export async function tryRestoreFolder(
  openFile: (content: string, name: string | null, uri: string) => void
): Promise<boolean> {
  const folderUri = Storage.getString(StorageKeys.LAST_FOLDER_URI);
  if (!folderUri) return false;

  try {
    const result = await restoreFolder(folderUri);
    if (result) {
      openFolderResult(result, openFile);
      return true;
    }
  } catch {
    // Permission expired or folder gone — clear stored URI
    Storage.setString(StorageKeys.LAST_FOLDER_URI, '');
  }
  return false;
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
