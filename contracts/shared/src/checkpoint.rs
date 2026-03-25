/// A point-in-time snapshot of a value.
pub struct Checkpoint<T> {
    pub ledger: u32,
    pub value: T,
}

/// Storage backend for a checkpoint timeline.
/// Implement on a struct that holds env + key context.
pub trait CheckpointStore {
    type Value: Default + Clone;

    fn count(&self) -> u32;
    fn set_count(&self, count: u32);
    fn get(&self, index: u32) -> Checkpoint<Self::Value>;
    fn set(&self, index: u32, cp: Checkpoint<Self::Value>);
    fn current_ledger(&self) -> u32;
}

/// Push a new value at the current ledger.
/// If the latest checkpoint is at the same ledger, updates in place.
pub fn push<S: CheckpointStore>(store: &S, value: S::Value) {
    let count = store.count();
    let ledger = store.current_ledger();

    if count > 0 {
        let last = store.get(count - 1);
        if last.ledger == ledger {
            store.set(count - 1, Checkpoint { ledger, value });
            return;
        }
    }

    store.set(count, Checkpoint { ledger, value });
    store.set_count(count + 1);
}

/// Look up the value at or before `target_ledger`.
/// Returns `Value::default()` if no checkpoint exists at or before the target.
pub fn lookup_at<S: CheckpointStore>(store: &S, target_ledger: u32) -> S::Value {
    let count = store.count();
    if count == 0 {
        return S::Value::default();
    }

    // Fast path: latest checkpoint is at or before target
    let last = store.get(count - 1);
    if last.ledger <= target_ledger {
        return last.value;
    }

    // All checkpoints are after target
    let first = store.get(0);
    if first.ledger > target_ledger {
        return S::Value::default();
    }

    // Binary search: find last checkpoint with ledger <= target
    let mut lo: u32 = 0;
    let mut hi: u32 = count - 1;
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if store.get(mid).ledger <= target_ledger {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    store.get(lo).value
}

/// Get the latest value, or `Value::default()` if no checkpoints exist.
pub fn latest<S: CheckpointStore>(store: &S) -> S::Value {
    let count = store.count();
    if count == 0 {
        S::Value::default()
    } else {
        store.get(count - 1).value
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    extern crate std;

    use super::*;
    use alloc::vec::Vec;
    use std::cell::RefCell;

    /// In-memory CheckpointStore for unit testing — no Soroban dependency.
    struct MemStore {
        entries: RefCell<Vec<(u32, u64)>>,
        ledger: RefCell<u32>,
    }

    impl MemStore {
        fn new(ledger: u32) -> Self {
            Self {
                entries: RefCell::new(Vec::new()),
                ledger: RefCell::new(ledger),
            }
        }

        fn set_ledger(&self, ledger: u32) {
            *self.ledger.borrow_mut() = ledger;
        }
    }

    impl CheckpointStore for MemStore {
        type Value = u64;

        fn count(&self) -> u32 {
            self.entries.borrow().len() as u32
        }

        fn set_count(&self, count: u32) {
            self.entries.borrow_mut().truncate(count as usize);
        }

        fn get(&self, index: u32) -> Checkpoint<u64> {
            let entries = self.entries.borrow();
            let (ledger, value) = entries[index as usize];
            Checkpoint { ledger, value }
        }

        fn set(&self, index: u32, cp: Checkpoint<u64>) {
            let mut entries = self.entries.borrow_mut();
            let idx = index as usize;
            if idx == entries.len() {
                entries.push((cp.ledger, cp.value));
            } else {
                entries[idx] = (cp.ledger, cp.value);
            }
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

    // ── single checkpoint ───────────────────────────────────────────

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

    // ── two checkpoints ─────────────────────────────────────────────

    #[test]
    fn test_two_checkpoints_latest() {
        let store = MemStore::new(10);
        push(&store, 100);
        store.set_ledger(20);
        push(&store, 200);
        assert_eq!(store.count(), 2);
        assert_eq!(latest(&store), 200);
    }

    #[test]
    fn test_two_checkpoints_lookup_boundaries() {
        let store = MemStore::new(10);
        push(&store, 100);
        store.set_ledger(20);
        push(&store, 200);

        assert_eq!(lookup_at(&store, 9), 0);   // before first
        assert_eq!(lookup_at(&store, 10), 100); // exact first
        assert_eq!(lookup_at(&store, 15), 100); // between
        assert_eq!(lookup_at(&store, 19), 100); // just before second
        assert_eq!(lookup_at(&store, 20), 200); // exact second
        assert_eq!(lookup_at(&store, 100), 200); // after last
    }

    // ── many checkpoints (exercises binary search) ──────────────────

    #[test]
    fn test_many_checkpoints_sequential() {
        let store = MemStore::new(0);
        // Push checkpoints at ledgers 10, 20, 30, ..., 100
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

        // Between checkpoints
        assert_eq!(lookup_at(&store, 15), 10); // between 10 and 20
        assert_eq!(lookup_at(&store, 55), 50); // between 50 and 60
        assert_eq!(lookup_at(&store, 99), 90); // between 90 and 100

        // After all
        assert_eq!(lookup_at(&store, 101), 100);
        assert_eq!(lookup_at(&store, 1000), 100);
    }

    #[test]
    fn test_many_checkpoints_odd_count() {
        let store = MemStore::new(0);
        // 7 checkpoints — odd count to test binary search centering
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
}
