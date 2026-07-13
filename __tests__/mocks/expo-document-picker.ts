// Stub for expo-document-picker (avoids pulling in expo-modules-core in tests).
export function getDocumentAsync(_options?: unknown): Promise<{canceled: boolean; assets: null}> {
  return Promise.resolve({canceled: true, assets: null});
}
