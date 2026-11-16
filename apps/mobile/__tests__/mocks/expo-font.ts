// Stub for expo-font (avoids pulling in expo-modules-core in tests).
export function isLoaded(_name: string): boolean {
  return false;
}
export function loadAsync(_map: Record<string, string>): Promise<void> {
  return Promise.resolve();
}
