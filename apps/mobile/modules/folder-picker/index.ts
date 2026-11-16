import { requireNativeModule } from 'expo-modules-core';

export type FolderFile = {
  name: string;
  uri: string;
  relativePath: string; // path relative to folder root, e.g. "subdir/file.md"
};

export type FolderReference = {
  folderUri: string;
  folderName: string;
  bookmarkKey?: string; // iOS; Android restores using its persisted tree URI
};

export type FolderResult = FolderReference & {
  files: FolderFile[];
};

const FolderPickerModule = requireNativeModule('FolderPicker');

/**
 * Open a system folder picker. Returns all .md / .txt files in the folder
 * (recursively), sorted by path.
 */
export function pickFolder(): Promise<FolderResult | null> {
  return FolderPickerModule.pickFolder();
}

/**
 * Restore access using the iOS bookmark key or Android tree URI.
 * Returns null if the permission is no longer valid (user revoked, folder deleted).
 */
export function restoreFolder(handle: string): Promise<FolderResult | null> {
  return FolderPickerModule.restoreFolder(handle);
}

export function readFolderFile(bookmarkKey: string, relativePath: string): Promise<string> {
  return FolderPickerModule.readFolderFile(bookmarkKey, relativePath);
}

export function statFolderFile(bookmarkKey: string, relativePath: string): Promise<number | null> {
  return FolderPickerModule.statFolderFile(bookmarkKey, relativePath);
}

/** iOS only. Native code pauses the presenter while the app is backgrounded. */
export function observeFolder(bookmarkKey: string, onChange: () => void): () => void {
  const subscription = FolderPickerModule.addListener('onFolderChanged', (event: {bookmarkKey: string}) => {
    if (event.bookmarkKey === bookmarkKey) onChange();
  });
  const watch = (key: string | null) => {
    void FolderPickerModule.watchFolder(key).catch((error: unknown) => {
      console.warn('Could not set the folder presenter:', error);
    });
  };
  watch(bookmarkKey);
  return () => {
    subscription.remove();
    watch(null);
  };
}
