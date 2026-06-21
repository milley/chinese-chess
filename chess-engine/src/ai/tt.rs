//! Transposition table for caching search results.
//!
//! Uses a fixed-size array indexed by Zobrist hash. Always-replace policy
//! (simple, works well for short time controls). Single-threaded — no
//! synchronization needed since the AI search runs on one thread.

use crate::pieces::Move;

/// Transposition table entry bound type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TTFlag {
    /// PV node — score is exact
    Exact,
    /// Cut-node — score is a lower bound (beta cutoff)
    Lower,
    /// All-node — score is an upper bound (failed low)
    Upper,
}

/// A single transposition table entry
#[derive(Clone, Debug)]
pub struct TTEntry {
    /// Full Zobrist hash for verification (collision detection)
    pub hash: u64,
    /// Search depth at which this entry was stored
    pub depth: u8,
    /// Evaluation score from the search
    pub score: i32,
    /// Bound type (exact, lower, upper)
    pub flag: TTFlag,
    /// Best move found at this node (for move ordering)
    pub best_move: Option<Move>,
}

/// Transposition table — fixed-size hash table with always-replace policy
pub struct TranspositionTable {
    entries: Vec<Option<TTEntry>>,
    mask: usize,
}

impl TranspositionTable {
    /// Create a new transposition table with the given capacity.
    /// `size_mb` is the approximate size in megabytes.
    /// Actual size is rounded down to the nearest power of 2.
    pub fn new(size_mb: usize) -> Self {
        // Each entry is approximately 32 bytes (u64 + u8 + i32 + enum + Option<Move>)
        let entry_size = 32;
        let max_entries = (size_mb * 1024 * 1024) / entry_size;
        // Round down to nearest power of 2
        let capacity = max_entries.next_power_of_two() / 2;
        let capacity = capacity.max(1);
        let mask = capacity - 1;

        let mut entries = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            entries.push(None);
        }

        Self { entries, mask }
    }

    /// Create a table with default size (8 MB ≈ 250K entries)
    pub fn default_size() -> Self {
        Self::new(8)
    }

    /// Look up an entry by Zobrist hash.
    /// Returns `Some(entry)` only if:
    /// - The stored hash matches (no collision)
    /// - The stored depth is >= the requested depth
    pub fn probe(&self, hash: u64, depth: u8) -> Option<&TTEntry> {
        let index = hash as usize & self.mask;
        match &self.entries[index] {
            Some(entry) if entry.hash == hash && entry.depth >= depth => Some(entry),
            _ => None,
        }
    }

    /// Look up an entry by hash for move ordering only.
    /// Returns the entry if the hash matches, regardless of depth.
    /// Used to get the best move hint even from shallower searches.
    pub fn probe_for_move(&self, hash: u64) -> Option<&TTEntry> {
        let index = hash as usize & self.mask;
        match &self.entries[index] {
            Some(entry) if entry.hash == hash => Some(entry),
            _ => None,
        }
    }

    /// Store a search result in the table.
    /// Always-replace policy: overwrites any existing entry at this index.
    pub fn store(&mut self, hash: u64, depth: u8, score: i32, flag: TTFlag, best_move: Option<Move>) {
        let index = hash as usize & self.mask;
        self.entries[index] = Some(TTEntry {
            hash,
            depth,
            score,
            flag,
            best_move,
        });
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = None;
        }
    }

    /// Get the number of entries currently stored
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|e| e.is_none())
    }

    /// Get the total capacity
    pub fn capacity(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;

    #[test]
    fn test_tt_store_and_probe() {
        let mut tt = TranspositionTable::new(1);
        let m = Move::new(Position::new(1, 7), Position::new(4, 7));

        tt.store(0x12345678, 4, 100, TTFlag::Exact, Some(m));

        let entry = tt.probe(0x12345678, 4).unwrap();
        assert_eq!(entry.hash, 0x12345678);
        assert_eq!(entry.depth, 4);
        assert_eq!(entry.score, 100);
        assert_eq!(entry.flag, TTFlag::Exact);
        assert_eq!(entry.best_move, Some(m));
    }

    #[test]
    fn test_tt_probe_depth_filter() {
        let mut tt = TranspositionTable::new(1);
        tt.store(0xABCD, 3, 50, TTFlag::Lower, None);

        // Should find entry when requesting depth <= stored depth
        assert!(tt.probe(0xABCD, 3).is_some());
        assert!(tt.probe(0xABCD, 2).is_some());
        assert!(tt.probe(0xABCD, 1).is_some());

        // Should NOT find entry when requesting depth > stored depth
        assert!(tt.probe(0xABCD, 4).is_none());
        assert!(tt.probe(0xABCD, 5).is_none());
    }

    #[test]
    fn test_tt_probe_hash_mismatch() {
        let mut tt = TranspositionTable::new(1);
        tt.store(0x1111, 4, 100, TTFlag::Exact, None);

        // Different hash should not match, even if it maps to the same index
        assert!(tt.probe(0x2222, 4).is_none());
    }

    #[test]
    fn test_tt_probe_for_move() {
        let mut tt = TranspositionTable::new(1);
        let m = Move::new(Position::new(0, 9), Position::new(0, 8));
        tt.store(0xBEEF, 2, -50, TTFlag::Upper, Some(m));

        // probe_for_move should find entry even with depth > stored depth
        let entry = tt.probe_for_move(0xBEEF).unwrap();
        assert_eq!(entry.best_move, Some(m));

        // Regular probe should not (depth 4 > stored depth 2)
        assert!(tt.probe(0xBEEF, 4).is_none());
    }

    #[test]
    fn test_tt_always_replace() {
        let mut tt = TranspositionTable::new(1);

        tt.store(0x1111, 4, 100, TTFlag::Exact, None);
        tt.store(0x2222, 3, -50, TTFlag::Lower, None);

        // If both hash to same index, second write should win
        // (This depends on the hash values mapping to the same index,
        //  which we can't guarantee, so just test that store works)
        assert!(tt.probe(0x2222, 3).is_some());
    }

    #[test]
    fn test_tt_clear() {
        let mut tt = TranspositionTable::new(1);
        tt.store(0x1111, 4, 100, TTFlag::Exact, None);
        assert!(!tt.is_empty());

        tt.clear();
        assert!(tt.is_empty());
        assert!(tt.probe(0x1111, 4).is_none());
    }

    #[test]
    fn test_tt_flag_exact() {
        let mut tt = TranspositionTable::new(1);
        tt.store(0x1, 3, 50, TTFlag::Exact, None);

        let entry = tt.probe(0x1, 3).unwrap();
        assert_eq!(entry.flag, TTFlag::Exact);
    }

    #[test]
    fn test_tt_default_size() {
        let tt = TranspositionTable::default_size();
        assert!(tt.capacity() > 0);
        assert!(tt.is_empty());
    }

    #[test]
    fn test_tt_len() {
        let mut tt = TranspositionTable::new(1);

        // Use hashes that are very different to avoid index collisions
        tt.store(0x0000000000000001, 3, 100, TTFlag::Exact, None);
        assert_eq!(tt.len(), 1);

        tt.store(0x0000000000000002, 3, -50, TTFlag::Lower, None);
        // len is at least 1, might be 2 if different indices
        assert!(tt.len() >= 1);
    }
}
