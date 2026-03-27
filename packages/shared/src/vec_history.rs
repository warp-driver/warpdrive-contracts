extern crate alloc;

use alloc::vec::Vec;

/// A point-in-time snapshot of a value.
#[derive(Clone)]
pub struct Entry<T> {
    pub ledger: u32,
    pub value: T,
}

/// Storage backend for a pruned history timeline.
///
/// Implement on a struct that holds env + key context.
/// Entries are sorted by ledger. On every push, entries older
/// than `cutoff()` blocks are pruned down to at most one.
pub trait VecHistoryStore {
    type Value: Default + Clone;

    /// Maximum age (in ledgers) before old entries are pruned.
    fn cutoff(&self) -> u32;

    fn load(&self) -> Vec<Entry<Self::Value>>;
    fn save(&self, entries: Vec<Entry<Self::Value>>);
    fn current_ledger(&self) -> u32;
}

/// Push a new value at the current ledger.
/// If the latest entry is at the same ledger, updates in place.
/// Then prunes entries older than the cutoff, keeping at most one.
pub fn push<S: VecHistoryStore>(store: &S, value: S::Value) {
    let mut entries = store.load();
    let ledger = store.current_ledger();

    if let Some(last) = entries.last_mut() {
        if last.ledger == ledger {
            last.value = value;
            prune(&mut entries, ledger, store.cutoff());
            store.save(entries);
            return;
        }
    }

    entries.push(Entry { ledger, value });
    prune(&mut entries, ledger, store.cutoff());
    store.save(entries);
}

/// Prune entries older than `current_ledger - cutoff`, keeping exactly one.
fn prune<T: Default + Clone>(entries: &mut Vec<Entry<T>>, ledger: u32, cutoff: u32) {
    if entries.len() <= 1 {
        return;
    }

    let cutoff_ledger = ledger.saturating_sub(cutoff);

    // Entries are sorted by ledger. Count how many are strictly before cutoff.
    let old_count = entries
        .iter()
        .take_while(|e| e.ledger < cutoff_ledger)
        .count();

    // Keep exactly one old entry (the most recent), remove the rest.
    if old_count >= 2 {
        entries.drain(..old_count - 1);
    }
}

/// Look up the value at or before `target_ledger`.
/// Returns `Value::default()` if no entry exists at or before the target.
pub fn lookup_at<S: VecHistoryStore>(store: &S, target_ledger: u32) -> S::Value {
    let entries = store.load();

    if entries.is_empty() {
        return S::Value::default();
    }

    // Fast path: latest entry is at or before target
    if entries.last().unwrap().ledger <= target_ledger {
        return entries.last().unwrap().value.clone();
    }

    // All entries are after target
    if entries[0].ledger > target_ledger {
        return S::Value::default();
    }

    // Binary search: find last entry with ledger <= target
    let mut lo = 0usize;
    let mut hi = entries.len() - 1;
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if entries[mid].ledger <= target_ledger {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    entries[lo].value.clone()
}

/// Get the latest value, or `Value::default()` if no entries exist.
pub fn latest<S: VecHistoryStore>(store: &S) -> S::Value {
    let entries = store.load();
    match entries.last() {
        Some(e) => e.value.clone(),
        None => S::Value::default(),
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::cell::RefCell;

    /// In-memory VecHistoryStore for unit testing — no Soroban dependency.
    struct MemStore {
        entries: RefCell<Vec<Entry<u64>>>,
        ledger: RefCell<u32>,
        cutoff: u32,
    }

    impl MemStore {
        fn new(ledger: u32) -> Self {
            Self {
                entries: RefCell::new(Vec::new()),
                ledger: RefCell::new(ledger),
                cutoff: u32::MAX, // no pruning by default
            }
        }

        fn with_cutoff(ledger: u32, cutoff: u32) -> Self {
            Self {
                entries: RefCell::new(Vec::new()),
                ledger: RefCell::new(ledger),
                cutoff,
            }
        }

        fn set_ledger(&self, ledger: u32) {
            *self.ledger.borrow_mut() = ledger;
        }

        fn count(&self) -> usize {
            self.entries.borrow().len()
        }
    }

    impl VecHistoryStore for MemStore {
        type Value = u64;

        fn cutoff(&self) -> u32 {
            self.cutoff
        }

        fn load(&self) -> Vec<Entry<u64>> {
            self.entries.borrow().clone()
        }

        fn save(&self, entries: Vec<Entry<u64>>) {
            *self.entries.borrow_mut() = entries;
        }

        fn current_ledger(&self) -> u32 {
            *self.ledger.borrow()
        }
    }

    // ── empty store ─────────────────────────────────────────────────

    #[test]
    fn test_latest_empty() {
        let store = MemStore::new(0);
        assert_eq!(latest(&store), 0);
    }

    #[test]
    fn test_lookup_at_empty() {
        let store = MemStore::new(0);
        assert_eq!(lookup_at(&store, 0), 0);
        assert_eq!(lookup_at(&store, 100), 0);
        assert_eq!(lookup_at(&store, u32::MAX), 0);
    }

    // ── single entry ────────────────────────────────────────────────

    #[test]
    fn test_single_push_and_latest() {
        let store = MemStore::new(10);
        push(&store, 42);
        assert_eq!(latest(&store), 42);
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_single_lookup_exact() {
        let store = MemStore::new(10);
        push(&store, 42);
        assert_eq!(lookup_at(&store, 10), 42);
    }

    #[test]
    fn test_single_lookup_after() {
        let store = MemStore::new(10);
        push(&store, 42);
        assert_eq!(lookup_at(&store, 100), 42);
        assert_eq!(lookup_at(&store, u32::MAX), 42);
    }

    #[test]
    fn test_single_lookup_before() {
        let store = MemStore::new(10);
        push(&store, 42);
        assert_eq!(lookup_at(&store, 9), 0);
        assert_eq!(lookup_at(&store, 0), 0);
    }

    // ── same-ledger coalescing ──────────────────────────────────────

    #[test]
    fn test_push_same_ledger_overwrites() {
        let store = MemStore::new(10);
        push(&store, 100);
        push(&store, 200);
        push(&store, 300);
        assert_eq!(store.count(), 1); // still one entry
        assert_eq!(latest(&store), 300);
        assert_eq!(lookup_at(&store, 10), 300);
    }

    // ── two entries ─────────────────────────────────────────────────

    #[test]
    fn test_two_entries_latest() {
        let store = MemStore::new(10);
        push(&store, 100);
        store.set_ledger(20);
        push(&store, 200);
        assert_eq!(store.count(), 2);
        assert_eq!(latest(&store), 200);
    }

    #[test]
    fn test_two_entries_lookup_boundaries() {
        let store = MemStore::new(10);
        push(&store, 100);
        store.set_ledger(20);
        push(&store, 200);

        assert_eq!(lookup_at(&store, 9), 0); // before first
        assert_eq!(lookup_at(&store, 10), 100); // exact first
        assert_eq!(lookup_at(&store, 15), 100); // between
        assert_eq!(lookup_at(&store, 19), 100); // just before second
        assert_eq!(lookup_at(&store, 20), 200); // exact second
        assert_eq!(lookup_at(&store, 100), 200); // after last
    }

    // ── many entries (exercises binary search) ──────────────────────

    #[test]
    fn test_many_entries_sequential() {
        let store = MemStore::new(0);
        // Push entries at ledgers 10, 20, 30, ..., 100
        for i in 1..=10u32 {
            store.set_ledger(i * 10);
            push(&store, (i * 10) as u64);
        }
        assert_eq!(store.count(), 10);

        // Before all
        assert_eq!(lookup_at(&store, 0), 0);
        assert_eq!(lookup_at(&store, 9), 0);

        // Exact hits
        for i in 1..=10u32 {
            assert_eq!(lookup_at(&store, i * 10), (i * 10) as u64);
        }

        // Between entries
        assert_eq!(lookup_at(&store, 15), 10); // between 10 and 20
        assert_eq!(lookup_at(&store, 55), 50); // between 50 and 60
        assert_eq!(lookup_at(&store, 99), 90); // between 90 and 100

        // After all
        assert_eq!(lookup_at(&store, 101), 100);
        assert_eq!(lookup_at(&store, 1000), 100);
    }

    #[test]
    fn test_many_entries_odd_count() {
        let store = MemStore::new(0);
        // 7 entries — odd count to test binary search centering
        for i in 1..=7u32 {
            store.set_ledger(i * 100);
            push(&store, i as u64);
        }
        assert_eq!(store.count(), 7);

        assert_eq!(lookup_at(&store, 99), 0);
        assert_eq!(lookup_at(&store, 100), 1);
        assert_eq!(lookup_at(&store, 350), 3);
        assert_eq!(lookup_at(&store, 400), 4);
        assert_eq!(lookup_at(&store, 700), 7);
        assert_eq!(lookup_at(&store, 999), 7);
    }

    // ── value can go to zero ────────────────────────────────────────

    #[test]
    fn test_push_zero_value() {
        let store = MemStore::new(10);
        push(&store, 100);
        store.set_ledger(20);
        push(&store, 0);

        assert_eq!(lookup_at(&store, 10), 100);
        assert_eq!(lookup_at(&store, 15), 100);
        assert_eq!(lookup_at(&store, 20), 0);
        assert_eq!(lookup_at(&store, 30), 0);
        assert_eq!(latest(&store), 0);
    }

    // ── non-monotonic values ────────────────────────────────────────

    #[test]
    fn test_values_go_up_and_down() {
        let store = MemStore::new(10);
        push(&store, 100);
        store.set_ledger(20);
        push(&store, 300);
        store.set_ledger(30);
        push(&store, 50);
        store.set_ledger(40);
        push(&store, 200);

        assert_eq!(lookup_at(&store, 10), 100);
        assert_eq!(lookup_at(&store, 20), 300);
        assert_eq!(lookup_at(&store, 25), 300);
        assert_eq!(lookup_at(&store, 30), 50);
        assert_eq!(lookup_at(&store, 40), 200);
    }

    // ── coalescing interleaved with advances ─────────────────────────

    #[test]
    fn test_coalesce_then_advance() {
        let store = MemStore::new(10);
        push(&store, 100);
        push(&store, 200); // coalesces at ledger 10
        store.set_ledger(20);
        push(&store, 300);
        push(&store, 400); // coalesces at ledger 20

        assert_eq!(store.count(), 2);
        assert_eq!(lookup_at(&store, 10), 200);
        assert_eq!(lookup_at(&store, 20), 400);
    }

    // ── large gap between ledgers ───────────────────────────────────

    #[test]
    fn test_large_ledger_gap() {
        let store = MemStore::new(1);
        push(&store, 10);
        store.set_ledger(1_000_000);
        push(&store, 20);

        assert_eq!(lookup_at(&store, 0), 0);
        assert_eq!(lookup_at(&store, 1), 10);
        assert_eq!(lookup_at(&store, 500_000), 10);
        assert_eq!(lookup_at(&store, 999_999), 10);
        assert_eq!(lookup_at(&store, 1_000_000), 20);
        assert_eq!(lookup_at(&store, u32::MAX), 20);
    }

    // ── consecutive ledgers ─────────────────────────────────────────

    #[test]
    fn test_consecutive_ledgers() {
        let store = MemStore::new(0);
        for i in 1..=5u32 {
            store.set_ledger(i);
            push(&store, (i * 10) as u64);
        }
        assert_eq!(store.count(), 5);

        assert_eq!(lookup_at(&store, 0), 0);
        assert_eq!(lookup_at(&store, 1), 10);
        assert_eq!(lookup_at(&store, 2), 20);
        assert_eq!(lookup_at(&store, 3), 30);
        assert_eq!(lookup_at(&store, 4), 40);
        assert_eq!(lookup_at(&store, 5), 50);
        assert_eq!(lookup_at(&store, 6), 50);
    }

    // ── latest after overwrite ──────────────────────────────────────

    #[test]
    fn test_latest_reflects_overwrite() {
        let store = MemStore::new(10);
        push(&store, 111);
        assert_eq!(latest(&store), 111);
        push(&store, 222); // same ledger overwrite
        assert_eq!(latest(&store), 222);
        store.set_ledger(20);
        push(&store, 333);
        assert_eq!(latest(&store), 333);
    }

    // ── pruning behaviour ───────────────────────────────────────────

    #[test]
    fn test_prune_keeps_one_old_entry() {
        let store = MemStore::with_cutoff(100, 200);
        push(&store, 1); // ledger 100
        store.set_ledger(200);
        push(&store, 2); // ledger 200
        store.set_ledger(300);
        push(&store, 3); // ledger 300
        store.set_ledger(400);
        push(&store, 4); // ledger 400

        // cutoff_ledger = 400 - 200 = 200
        // Entries at 100 have ledger < 200 → old. Only one, so kept.
        assert_eq!(store.count(), 4);

        store.set_ledger(500);
        push(&store, 5); // ledger 500
        // cutoff_ledger = 500 - 200 = 300
        // Old entries: ledger 100, ledger 200 → 2 old entries
        // Keep only ledger 200, remove ledger 100
        assert_eq!(store.count(), 4); // was 5, pruned 1

        assert_eq!(lookup_at(&store, 200), 2);
        assert_eq!(lookup_at(&store, 300), 3);
        assert_eq!(lookup_at(&store, 500), 5);
    }

    #[test]
    fn test_prune_removes_multiple_old_entries() {
        let store = MemStore::with_cutoff(10, 50);

        // Push entries at ledgers 10, 20, 30, 40, 50
        for i in 1..=5u32 {
            store.set_ledger(i * 10);
            push(&store, (i * 10) as u64);
        }
        assert_eq!(store.count(), 5);

        // Jump far ahead: cutoff_ledger = 500 - 50 = 450
        // All 5 entries (10..50) are < 450 → 5 old entries → keep 1
        store.set_ledger(500);
        push(&store, 999);
        // Kept: ledger 50 (most recent old) + ledger 500 (new)
        assert_eq!(store.count(), 2);

        assert_eq!(lookup_at(&store, 50), 50); // the preserved old entry
        assert_eq!(lookup_at(&store, 499), 50);
        assert_eq!(lookup_at(&store, 500), 999);
    }

    #[test]
    fn test_prune_no_pruning_when_within_cutoff() {
        let store = MemStore::with_cutoff(10, 200);

        for i in 1..=5u32 {
            store.set_ledger(i * 10);
            push(&store, (i * 10) as u64);
        }
        // Ledger 50, cutoff_ledger = 50 - 200 = 0 (saturating)
        // No entries have ledger < 0, no pruning
        assert_eq!(store.count(), 5);
    }

    #[test]
    fn test_prune_on_coalesce() {
        let store = MemStore::with_cutoff(10, 50);
        push(&store, 1); // ledger 10
        store.set_ledger(20);
        push(&store, 2); // ledger 20

        // Jump ahead, push twice at same ledger (coalesce + prune)
        store.set_ledger(300);
        push(&store, 30);
        // cutoff = 300 - 50 = 250, old: ledger 10 and 20 → keep 20
        assert_eq!(store.count(), 2); // (20, 2) and (300, 30)

        push(&store, 31); // coalesces at 300
        assert_eq!(store.count(), 2);
        assert_eq!(latest(&store), 31);
        assert_eq!(lookup_at(&store, 20), 2);
    }

    #[test]
    fn test_prune_preserves_lookups_within_cutoff() {
        let store = MemStore::with_cutoff(100, 100);

        // Build up history: 100, 150, 200, 250
        push(&store, 10); // ledger 100
        store.set_ledger(150);
        push(&store, 15);
        store.set_ledger(200);
        push(&store, 20);
        store.set_ledger(250);
        push(&store, 25);

        // cutoff_ledger = 250 - 100 = 150
        // Old entries: ledger 100 has ledger < 150 → 1 old entry, kept
        assert_eq!(store.count(), 4);

        // Everything within cutoff window is accessible
        assert_eq!(lookup_at(&store, 150), 15);
        assert_eq!(lookup_at(&store, 175), 15);
        assert_eq!(lookup_at(&store, 200), 20);
        assert_eq!(lookup_at(&store, 250), 25);
        // The preserved old entry still covers before-cutoff lookups
        assert_eq!(lookup_at(&store, 100), 10);
        assert_eq!(lookup_at(&store, 130), 10);
    }

    #[test]
    fn test_prune_with_single_entry_no_panic() {
        let store = MemStore::with_cutoff(10, 5);
        push(&store, 1); // ledger 10
        store.set_ledger(1000);
        // Only one entry (old), pruning should not remove it
        push(&store, 2);
        // ledger 10 < 995, but it's the only old entry → kept
        assert_eq!(store.count(), 2);
        assert_eq!(lookup_at(&store, 10), 1);
        assert_eq!(lookup_at(&store, 1000), 2);
    }
}
