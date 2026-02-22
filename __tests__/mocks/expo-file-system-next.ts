// In-memory mock for expo-file-system/next

const fileStore = new Map<string, string>();

export class File {
  uri: string;
  constructor(...pathParts: (string | Directory)[]) {
    this.uri = pathParts.map(p => typeof p === 'string' ? p : p.uri).join('/');
  }
  get exists(): boolean {
    return fileStore.has(this.uri);
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

// Helper to reset state between tests
export function _resetFileStore(): void {
  fileStore.clear();
}

export function _getFileStore(): Map<string, string> {
  return fileStore;
}
