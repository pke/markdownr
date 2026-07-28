import {File, Directory, Paths} from 'expo-file-system/next';
import {Storage, StorageKeys} from './settings';
import {parseFrontMatter} from './frontmatter';
import type {FileSource} from './fileChangeDetection';

const MAX_RECENT_FILES = 10;
const recentFilesDir = new Directory(Paths.cache, 'recent-files');

export interface RecentFileEntry {
  id: string;
  title: string;
  subtitle: string;
  addedAt: string;
  /** How to re-read the live file (Phase 2). Entries predating this field
   * simply open from the cache copy, as before. */
  source?: FileSource;
}

function ensureDir(): void {
  if (!recentFilesDir.exists) {
    recentFilesDir.create({intermediates: true});
  }
}

function deleteCachedFile(id: string): void {
  try { new File(recentFilesDir, `${id}.md`).delete(); } catch {}
}

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

const HEADING_REGEX = /^#{1,6}\s+(.+)$/m;

function deriveTitle(content: string, fileName: string | null): string {
  const {frontMatter} = parseFrontMatter(content);
  if (frontMatter?.title) return frontMatter.title;

  const headingMatch = content.match(HEADING_REGEX);
  if (headingMatch) return headingMatch[1].trim();

  if (fileName) return fileName.replace(/\.(md|markdown)$/i, '');

  return new Date().toISOString().split('T')[0];
}

function deriveSubtitle(fileName: string | null): string {
  if (fileName) return fileName;
  return new Date().toISOString().split('T')[0];
}

export function getRecentFiles(): RecentFileEntry[] {
  const raw = Storage.getString(StorageKeys.RECENT_FILES);
  if (!raw) return [];
  try {
    return JSON.parse(raw);
  } catch {
    return [];
  }
}

function saveRecentFiles(entries: RecentFileEntry[]): void {
  Storage.setString(StorageKeys.RECENT_FILES, JSON.stringify(entries));
}

export function addRecentFile(content: string, fileName: string | null, source?: FileSource): void {
  ensureDir();
  const entries = getRecentFiles();
  const subtitle = deriveSubtitle(fileName);

  const existingIndex = entries.findIndex(e => e.subtitle === subtitle);

  let id: string;
  if (existingIndex >= 0) {
    id = entries[existingIndex].id;
    entries.splice(existingIndex, 1);
  } else {
    id = generateId();
  }

  const title = deriveTitle(content, fileName);
  const newEntry: RecentFileEntry = {
    id,
    title,
    subtitle,
    addedAt: new Date().toISOString(),
    ...(source ? {source} : {}),
  };

  const file = new File(recentFilesDir, `${id}.md`);
  if (file.exists) file.delete();
  file.create();
  file.write(content);

  const updated = [newEntry, ...entries];
  const evicted = updated.slice(MAX_RECENT_FILES);
  evicted.forEach(e => deleteCachedFile(e.id));

  saveRecentFiles(updated.slice(0, MAX_RECENT_FILES));
}

export async function loadRecentFile(entry: RecentFileEntry): Promise<string | null> {
  try {
    const file = new File(recentFilesDir, `${entry.id}.md`);
    if (!file.exists) return null;
    return await file.text();
  } catch {
    return null;
  }
}

/** Modification time of a recents cache copy — the "when we last read it"
 * baseline for external-change detection after a launch-restore. */
export function getCachedFileMtime(id: string): number | null {
  try {
    const mtime = new File(recentFilesDir, `${id}.md`).modificationTime;
    return mtime && mtime > 0 ? mtime : null;
  } catch {
    return null;
  }
}

export function deleteRecentFile(id: string): void {
  const entries = getRecentFiles().filter(e => e.id !== id);
  saveRecentFiles(entries);
  deleteCachedFile(id);
}

export function clearAllRecentFiles(): void {
  getRecentFiles().forEach(e => deleteCachedFile(e.id));
  saveRecentFiles([]);
}
