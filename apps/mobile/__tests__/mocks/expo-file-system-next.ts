// In-memory mock for expo-file-system/next

const fileStore = new Map<string, string>();
const mtimeStore = new Map<string, number>();

export class File {
  uri: string;
  constructor(...pathParts: (string | Directory)[]) {
    this.uri = pathParts.map(p => typeof p === 'string' ? p : p.uri).join('/');
  }
  get exists(): boolean {
    return fileStore.has(this.uri);
  }
  get modificationTime(): number | null {
    return mtimeStore.get(this.uri) ?? null;
  }
  create(): void {
    if (!fileStore.has(this.uri)) {
      fileStore.set(this.uri, '');
    }
  }
  write(content: string): void {
    fileStore.set(this.uri, content);
  }
  async text(): Promise<string> {
    return fileStore.get(this.uri) ?? '';
  }
  delete(): void {
    fileStore.delete(this.uri);
  }
}

export class Directory {
  uri: string;
  constructor(...pathParts: string[]) {
    this.uri = pathParts.join('/');
  }
  get exists(): boolean {
    return true;
  }
  create(): void {}
}

export const Paths = {
  document: '/mock-documents',
  cache: '/mock-cache',
};

// Helpers to reset/seed state between tests
export function _resetFileStore(): void {
  fileStore.clear();
  mtimeStore.clear();
}

export function __setMtime(uri: string, mtime: number): void {
  mtimeStore.set(uri, mtime);
}

export function __clearMtimes(): void {
  mtimeStore.clear();
}

export function _getFileStore(): Map<string, string> {
  return fileStore;
}
