//! A map that keeps its most recently used entries and drops the oldest past a bound.

use std::collections::HashMap;
use std::hash::Hash;

/// Entries by key, each with when it was last used.
pub(super) struct Kept<K, V> {
    entries: HashMap<K, (V, u64)>,
    clock: u64,
}

impl<K, V> Default for Kept<K, V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            clock: 0,
        }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Kept<K, V> {
    /// The entry under `key`, now the most recently used.
    pub(super) fn get(&mut self, key: &K) -> Option<V> {
        self.clock += 1;
        let clock = self.clock;
        let (value, used) = self.entries.get_mut(key)?;
        *used = clock;
        Some(value.clone())
    }

    /// Keeps `value` under `key`, dropping the least recently used past `bound` entries.
    pub(super) fn put(&mut self, key: K, value: V, bound: usize) {
        self.clock += 1;
        self.entries.insert(key, (value, self.clock));
        while self.entries.len() > bound {
            let oldest = (self.entries.iter())
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone());
            match oldest {
                Some(key) => self.entries.remove(&key),
                None => break,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_least_recently_used_entry_goes_first_past_the_bound() {
        let mut kept = Kept::default();
        kept.put(1, "one", 2);
        kept.put(2, "two", 2);
        assert_eq!(kept.get(&1), Some("one"), "1 is now the latest used");
        kept.put(3, "three", 2);
        assert_eq!(kept.get(&2), None, "2 was used least recently");
        assert_eq!(kept.get(&1), Some("one"));
        assert_eq!(kept.get(&3), Some("three"));
    }
}
