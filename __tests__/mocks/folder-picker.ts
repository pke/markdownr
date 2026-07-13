// In-memory mock for the native folder-picker module (no native binding in tests).
export type FolderFile = {name: string; uri: string; relativePath: string};
export type FolderResult = {folderUri: string; folderName: string; files: FolderFile[]};

export function pickFolder(): Promise<FolderResult | null> {
  return Promise.resolve(null);
}

export function restoreFolder(_folderUri: string): Promise<FolderResult | null> {
  return Promise.resolve(null);
}
