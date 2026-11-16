import {restoreFolder, type FolderFile, type FolderReference, type FolderResult} from 'folder-picker';
import {readFileSource, sourceForUri, type FileSource} from './fileChangeDetection';

export type FolderLocation = {
  result: FolderResult;
  relativePath?: string;
};

export type ViewerDocument = [content: string, name: string | null, uri: string | null,
  source: FileSource | null, folder?: FolderLocation];
export type OpenDocument = (...document: ViewerDocument) => void;

export function folderHandle(folder: FolderReference): string {
  return folder.bookmarkKey ?? folder.folderUri;
}

export async function readFolder(handle: string): Promise<FolderResult | null> {
  try {
    return await restoreFolder(handle);
  } catch {
    return null;
  }
}

function escapeMarkdown(text: string): string {
  return text.replace(/[\\[\]*_`~<>]/g, '\\$&').replace(/[\r\n]+/g, ' ');
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
export function buildFolderIndex(files: FolderFile[], folderName: string): string {
  const lines = [`# ${escapeMarkdown(folderName)}`, ''];
  for (const file of files) {
    const label = escapeMarkdown(file.name.replace(/\.(md|markdown|txt)$/i, ''));
    const path = file.relativePath.split('/').map(segment =>
      encodeURIComponent(segment).replace(/[()]/g, char => char === '(' ? '%28' : '%29'),
    ).join('/');
    lines.push(`- [${label}](${path})`);
  }
  return lines.join('\n');
}

// SAF document IDs can be opaque. Resolve the link in the enumerated tree,
// then use the exact URI supplied for that document by the provider.
export function resolveFolderLink(href: string, {result, relativePath}: FolderLocation): FolderFile | null {
  if (/^(?:[#/]|[a-z][a-z0-9+.-]*:)/i.test(href)) return null;

  const target = relativePath ? relativePath.split('/').slice(0, -1) : [];
  const path = href.split(/[?#]/, 1)[0];
  if (!path || path.endsWith('/')) return null;

  try {
    for (const part of path.split('/')) {
      const segment = decodeURIComponent(part);
      if (/[\\/\0]/.test(segment)) return null;
      if (segment === '..') {
        if (target.pop() === undefined) return null;
      } else if (segment && segment !== '.') {
        target.push(segment);
      }
    }
  } catch {
    return null;
  }

  const targetPath = target.join('/');
  return result.files.find(file => file.relativePath === targetPath) ?? null;
}

export function folderIndex(result: FolderResult): ViewerDocument {
  const content = buildFolderIndex(sortFiles(result.files), result.folderName);
  return [content, result.folderName, null, null, {result}];
}

export function updateFolderLocation(location: FolderLocation | undefined, result: FolderResult): FolderLocation | null {
  if (!location || folderHandle(location.result) !== folderHandle(result)) return null;
  const previous = location.result;
  const unchanged = previous.folderUri === result.folderUri && previous.folderName === result.folderName &&
    previous.files.length === result.files.length && previous.files.every((file, i) => {
      const next = result.files[i];
      return file.uri === next.uri && file.relativePath === next.relativePath && file.name === next.name;
    });
  return unchanged ? null : {...location, result};
}

export async function readFolderDocument(result: FolderResult, file: FolderFile): Promise<ViewerDocument> {
  const {name, uri, relativePath} = file;
  const source: FileSource | null = result.bookmarkKey
    ? {uri, kind: 'folder', ref: result.bookmarkKey, relativePath}
    : sourceForUri(uri);
  if (!source) throw new Error('Unsupported folder document URI');
  const content = await readFileSource(source);
  return [content, name, uri, source, {result, relativePath}];
}

export async function readFolderDocumentOrIndex(result: FolderResult, file?: FolderFile): Promise<ViewerDocument> {
  if (file) {
    try {
      return await readFolderDocument(result, file);
    } catch {
      // The provider may remove the file between listing and opening it.
    }
  }
  return folderIndex(result);
}
