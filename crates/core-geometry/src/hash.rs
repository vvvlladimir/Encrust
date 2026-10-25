use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// `FxHash`, the hash `rustc` uses for keys of its own making. `SipHash` resists attacker-chosen
/// collisions, which geometry-derived keys are not, and there are millions.
#[derive(Default)]
pub struct FastHasher {
    hash: u64,
}

/// The odd constant `FxHash` multiplies by, from `rustc_hash`.
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FastHasher {
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.add(u64::from(byte));
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_i64(&mut self, value: i64) {
        self.add(value as u64);
    }

    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }

    /// Hash maps bucket by low bits, and an odd multiply makes bit `n` depend only on bits `0..=n`.
    fn finish(&self) -> u64 {
        (self.hash ^ (self.hash >> 32)).wrapping_mul(SEED)
    }
}

/// A map over keys the workspace made itself, hashed by [`FastHasher`].
pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

/// A set over keys the workspace made itself, hashed by [`FastHasher`].
pub type FastSet<K> = HashSet<K, BuildHasherDefault<FastHasher>>;
