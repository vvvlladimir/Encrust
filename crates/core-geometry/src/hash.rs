use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// `FxHash`, the hash `rustc` uses for keys of its own making.
///
/// The standard hasher is `SipHash`, which is built to resist collisions chosen by an
/// attacker. A grid cell or a lattice edge worked out from geometry of our own is not
/// that, and there are millions of them to hash.
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

    /// The state, with its high bits folded into its low ones.
    ///
    /// A hash map takes the bucket from the low bits, and multiplying by an odd constant
    /// leaves bit `n` of the product depending only on bits `0..=n` of the input: a key
    /// whose low bits barely move — a lattice edge inside one layer of tiles — would pile
    /// into a handful of buckets without this.
    fn finish(&self) -> u64 {
        (self.hash ^ (self.hash >> 32)).wrapping_mul(SEED)
    }
}

/// A map over keys the workspace made itself, hashed by [`FastHasher`].
pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

/// A set over keys the workspace made itself, hashed by [`FastHasher`].
pub type FastSet<K> = HashSet<K, BuildHasherDefault<FastHasher>>;
