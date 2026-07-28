// In-memory mock for modules/file-picker

import type {PickedFile} from '../../modules/file-picker';

const contents = new Map<string, string>();
const mtimes = new Map<string, number>();
let nextPick: PickedFile | null = null;

export type {PickedFile};

export function pickFile(): Promise<PickedFile | null> {
  return Promise.resolve(nextPick);
}

export function readPickedFile(ref: string): Promise<string> {
  const content = contents.get(ref);
  if (content === undefined) return Promise.reject(new Error(`READ_FAILED: ${ref}`));
  return Promise.resolve(content);
}

export function statPickedFile(ref: string): Promise<number | null> {
  return Promise.resolve(mtimes.get(ref) ?? null);
}

// --- test helpers ---
export function __setNextPick(pick: PickedFile | null): void {
  nextPick = pick;
}
export function __setPickedContent(ref: string, content: string): void {
  contents.set(ref, content);
}
export function __setPickedMtime(ref: string, mtime: number): void {
  mtimes.set(ref, mtime);
}
export function __resetFilePicker(): void {
  contents.clear();
  mtimes.clear();
  nextPick = null;
}
