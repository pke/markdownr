import {requireNativeModule} from 'expo-modules-core';

export type PickedFile = {
  /** URI of the real document (file:// on iOS, content:// on Android). */
  uri: string;
  name: string;
  /** iOS only: key for the persisted security-scoped bookmark. Use it as the
   * `ref` for readPickedFile/statPickedFile. On Android the uri is the ref. */
  bookmarkKey?: string;
};

const FilePickerModule = requireNativeModule('FilePicker');

/**
 * Open the system document picker IN PLACE (no cache copy): the picked file
 * stays live, so external edits are detectable. Returns null when cancelled.
 */
export function pickFile(): Promise<PickedFile | null> {
  return FilePickerModule.pickFile();
}

/** Read a picked file via its ref (iOS bookmarkKey / Android content uri). */
export function readPickedFile(ref: string): Promise<string> {
  return FilePickerModule.readPickedFile(ref);
}

/** Modification time (ms since epoch) or null when it cannot be trusted. */
export function statPickedFile(ref: string): Promise<number | null> {
  return FilePickerModule.statPickedFile(ref);
}
