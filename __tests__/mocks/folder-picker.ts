// In-memory mock for the native folder-picker module (no native binding in tests).
import type {FolderResult} from '../../modules/folder-picker';

export type {FolderFile, FolderResult} from '../../modules/folder-picker';

export const restoredFolders = new Map<string, FolderResult>();
export const folderContents = new Map<string, string>();
export const folderMtimes = new Map<string, number>();

export function pickFolder(): Promise<FolderResult | null> {
  return Promise.resolve(null);
}

export async function restoreFolder(handle: string): Promise<FolderResult | null> {
  return restoredFolders.get(handle) ?? null;
}

export async function readFolderFile(bookmark: string, path: string): Promise<string> {
  const content = folderContents.get(`${bookmark}/${path}`);
  if (content === undefined) throw new Error('Missing file');
  return content;
}

export async function statFolderFile(bookmark: string, path: string): Promise<number | null> {
  return folderMtimes.get(`${bookmark}/${path}`) ?? null;
}
