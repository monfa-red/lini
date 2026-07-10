//! First-appearance-ordered interning: dedup a value under a key, handing back a
//! stable index — the order `<defs>` ids get assigned, so output stays
//! deterministic [SPEC 17]. Shared by the paint tables (`gradient` / `hatch`)
//! and the filter table (`shadow`).

/// A keyed set of distinct values, in first-appearance order.
pub(crate) struct IdTable<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
}

impl<K: PartialEq, V> IdTable<K, V> {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Intern `value` under `key`, returning its stable 0-based index — reused
    /// when an equal key was already seen. `make` builds the value only on first
    /// sight.
    pub fn intern(&mut self, key: K, make: impl FnOnce() -> V) -> usize {
        if let Some(i) = self.keys.iter().position(|k| *k == key) {
            return i;
        }
        self.keys.push(key);
        self.values.push(make());
        self.values.len() - 1
    }

    /// The index a `key` interned to, if any.
    pub fn index_of(&self, key: &K) -> Option<usize> {
        self.keys.iter().position(|k| k == key)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[V] {
        &self.values
    }

    pub fn into_values(self) -> Vec<V> {
        self.values
    }
}

impl<K: PartialEq, V> Default for IdTable<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
