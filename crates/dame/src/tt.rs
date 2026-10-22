//! Transposition table.
//!
//! The table is an array of 64-byte clusters, one cache line each, holding
//! four 16-byte entries. An entry stores the full 64-bit key next to a packed
//! 64-bit payload; keeping key and payload in two words leaves room for a
//! lock-free "key XOR payload" scheme once the search runs on several threads.

use crate::moves::Move;

/// The kind of bound a stored score is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    /// The true score is at most the stored score (fail-low).
    Upper = 1,
    /// The true score is at least the stored score (fail-high).
    Lower = 2,
    /// The stored score is exact.
    Exact = 3,
}

/// A decoded table entry.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Hit {
    /// The compact best move, see [`compact`].
    pub(crate) mv: u16,
    pub(crate) score: i32,
    pub(crate) depth: i32,
    pub(crate) bound: Bound,
}

/// A 16-bit move fingerprint: origin and destination bits plus four bits of
/// the captured set, which separates captures that share their endpoints.
///
/// The fingerprint orders moves; it is matched against the generated legal
/// moves, so a rare collision costs move ordering, never correctness.
#[inline]
pub(crate) fn compact(mv: Move) -> u16 {
    let captured = (mv.captured().bits().wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 60) as u16;
    u16::from(mv.from().bit()) | u16::from(mv.to().bit()) << 6 | captured << 12
}

#[derive(Copy, Clone, Default)]
struct Entry {
    key: u64,
    /// `move:16 | score:16 | depth:8 | bound:8 | generation:8`, zero when empty.
    data: u64,
}

impl Entry {
    fn pack(mv: u16, score: i32, depth: i32, bound: Bound, generation: u8) -> u64 {
        debug_assert!(i16::try_from(score).is_ok());
        u64::from(mv)
            | u64::from(score as i16 as u16) << 16
            | u64::from(depth.clamp(0, 255) as u8) << 32
            | (bound as u64) << 40
            | u64::from(generation) << 48
    }

    fn is_empty(self) -> bool {
        self.data == 0
    }

    fn depth(self) -> i32 {
        i32::from((self.data >> 32) as u8)
    }

    fn generation(self) -> u8 {
        (self.data >> 48) as u8
    }

    fn hit(self) -> Hit {
        let bound = match (self.data >> 40) as u8 {
            1 => Bound::Upper,
            2 => Bound::Lower,
            _ => Bound::Exact,
        };
        Hit {
            mv: self.data as u16,
            score: i32::from((self.data >> 16) as u16 as i16),
            depth: self.depth(),
            bound,
        }
    }
}

#[derive(Copy, Clone, Default)]
#[repr(align(64))]
struct Cluster([Entry; 4]);

/// A fixed-size hash table of search results.
pub(crate) struct TranspositionTable {
    clusters: Vec<Cluster>,
    generation: u8,
}

impl TranspositionTable {
    /// A table using at most `megabytes` MiB (at least one cluster).
    pub(crate) fn new(megabytes: usize) -> Self {
        let bytes = megabytes.max(1) << 20;
        // The largest power of two that fits, so a cluster index is a mask.
        let count = 1 << (bytes / size_of::<Cluster>()).max(1).ilog2();
        Self {
            clusters: vec![Cluster::default(); count],
            generation: 1,
        }
    }

    /// Forgets all entries.
    pub(crate) fn clear(&mut self) {
        self.clusters.fill(Cluster::default());
        self.generation = 1;
    }

    /// Marks the start of a new search so that older entries are replaced first.
    pub(crate) fn new_search(&mut self) {
        // Generation 0 is reserved: an all-zero payload means "empty".
        self.generation = self.generation.wrapping_add(1).max(1);
    }

    #[inline]
    fn cluster(&self, key: u64) -> usize {
        // The table size is a power of two; the high bits are the best mixed.
        (key >> 32) as usize & (self.clusters.len() - 1)
    }

    /// Looks up `key`.
    #[inline]
    pub(crate) fn probe(&self, key: u64) -> Option<Hit> {
        self.clusters[self.cluster(key)]
            .0
            .iter()
            .find(|entry| entry.key == key && !entry.is_empty())
            .map(|entry| entry.hit())
    }

    /// Stores a result for `key`.
    ///
    /// An existing entry for the same key is overwritten unless it holds a
    /// deeper result; otherwise the stalest, then shallowest entry is replaced.
    pub(crate) fn store(&mut self, key: u64, mv: u16, score: i32, depth: i32, bound: Bound) {
        let generation = self.generation;
        let index = self.cluster(key);
        let cluster = &mut self.clusters[index].0;
        let data = Entry::pack(mv, score, depth, bound, generation);

        if let Some(entry) = cluster.iter_mut().find(|e| e.key == key && !e.is_empty()) {
            if depth >= entry.depth() || bound == Bound::Exact {
                // Keep the old best move if the new result has none.
                let mv = if mv == 0 { entry.hit().mv } else { mv };
                entry.data = Entry::pack(mv, score, depth, bound, generation);
            }
            return;
        }

        let age = |e: &Entry| generation.wrapping_sub(e.generation());
        let victim = cluster
            .iter_mut()
            .max_by_key(|e| (e.is_empty(), age(e), -e.depth()))
            .expect("clusters are non-empty");
        *victim = Entry { key, data };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_is_one_cache_line() {
        assert_eq!(size_of::<Cluster>(), 64);
        assert_eq!(TranspositionTable::new(1).clusters.len(), (1 << 20) / 64);
    }

    #[test]
    fn store_and_probe() {
        let mut tt = TranspositionTable::new(1);
        assert!(tt.probe(42).is_none());
        tt.store(42, 0x1234, -517, 7, Bound::Lower);
        let hit = tt.probe(42).unwrap();
        assert_eq!(
            (hit.mv, hit.score, hit.depth, hit.bound),
            (0x1234, -517, 7, Bound::Lower)
        );

        // A shallower result does not replace a deeper one.
        tt.store(42, 0x0042, 3, 2, Bound::Upper);
        assert_eq!(tt.probe(42).unwrap().depth, 7);

        tt.clear();
        assert!(tt.probe(42).is_none());
    }

    #[test]
    fn full_cluster_replaces_an_entry() {
        let mut tt = TranspositionTable::new(1);
        let keys: Vec<u64> = (0..5).map(|i| i << 8).collect();
        for (depth, &key) in (1..).zip(&keys) {
            tt.store(key, 1, 0, depth, Bound::Exact);
        }
        // All five keys map to the same cluster; the shallowest was evicted.
        assert!(tt.probe(keys[0]).is_none());
        assert!(keys[1..].iter().all(|&k| tt.probe(k).is_some()));
    }
}
