//! Markdownr: a map on a `Vec` with linear lookups, in place of std's `HashMap`, whose
//! hashing, random seed and table code cost the exe about 12 KB. The maps here hold
//! few keys (delimiter lengths, brace contexts, distinct reference labels).

#[derive(Clone, Debug)]
pub struct VecMap<K, V>(Vec<(K, V)>);

impl<K, V> Default for VecMap<K, V> {
    fn default() -> Self {
        VecMap(Vec::new())
    }
}

impl<K, V> VecMap<K, V> {
    pub fn get_by(&self, mut f: impl FnMut(&K) -> bool) -> Option<&V> {
        self.0.iter().find(|(k, _)| f(k)).map(|(_, v)| v)
    }

    pub fn get_mut_by(&mut self, mut f: impl FnMut(&K) -> bool) -> Option<&mut V> {
        self.0.iter_mut().find(|(k, _)| f(k)).map(|(_, v)| v)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter().map(|(k, v)| (k, v))
    }
}

impl<K: PartialEq, V> VecMap<K, V> {
    /// The value at `key`, inserted with `f()` if there is none.
    pub fn or_insert_with(&mut self, key: K, f: impl FnOnce() -> V) -> &mut V {
        let i = match self.0.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                self.0.push((key, f()));
                self.0.len() - 1
            }
        };
        &mut self.0[i].1
    }

    /// Set `key` to `value`, replacing an existing value.
    pub fn insert(&mut self, key: K, value: V) {
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => self.0.push((key, value)),
        }
    }
}
