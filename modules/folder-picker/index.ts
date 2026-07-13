import { requireNativeModule } from 'expo-modules-core';

export type FolderFile = {
  name: string;
  uri: string;
  relativePath: string; // path relative to folder root, e.g. "subdir/file.md"
};

export type FolderResult = {
  folderUri: string;  // persistent URI to bookmark/restore with
  folderName: string;
  files: FolderFile[];
};

const FolderPickerModule = requireNativeModule('FolderPicker');

/**
 * Open a system folder picker. Returns all .md / .txt files in the folder
 * (recursively), sorted by path. The folderUri can be passed to restoreFolder()
 * on subsequent launches to regain access without re-picking.
 */
export function pickFolder(): Promise<FolderResult | null> {
  return FolderPickerModule.pickFolder();
}

/**
 * Restore access to a previously picked folder using the persisted URI.
 * Returns null if the permission is no longer valid (user revoked, folder deleted).
 */
export function restoreFolder(folderUri: string): Promise<FolderResult | null> {
  return FolderPickerModule.restoreFolder(folderUri);
}
