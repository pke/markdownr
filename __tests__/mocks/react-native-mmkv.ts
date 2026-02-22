// In-memory mock for react-native-mmkv

const stores = new Map<string, Map<string, string>>();

export function createMMKV({id}: {id: string}) {
  if (!stores.has(id)) {
    stores.set(id, new Map());
  }
  const store = stores.get(id)!;
  return {
    getString(key: string): string | undefined {
      return store.get(key);
    },
    set(key: string, value: string | boolean | number): void {
      store.set(key, String(value));
    },
    getBoolean(key: string): boolean | undefined {
      const v = store.get(key);
      if (v === undefined) return undefined;
      return v === 'true';
    },
    delete(key: string): void {
      store.delete(key);
    },
    addOnValueChangedListener() {
      return {remove() {}};
    },
  };
}

export function _resetAllStores(): void {
  stores.forEach(s => s.clear());
}
