use super::{TranspositionTable, TtEntry};
use crate::position::Outcome;
use atomic_movegen::types::Move;

#[test]
fn tt_entry_size_is_reasonable() {
    let size = std::mem::size_of::<TtEntry>();
    assert!(size <= 128, "TtEntry size {size} exceeds 128 bytes");
}

#[test]
fn store_and_probe_solved_result() {
    let mut tt = TranspositionTable::with_mb(1);
    let key = 12345u64;
    let mv = Move::make_move(
        atomic_movegen::types::Square::A1,
        atomic_movegen::types::Square::A2,
    );

    tt.store(
        key,
        mv,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        crate::zobrist::INF,
        7,
        u32::MAX,
    );

    let entry = tt.probe(key).expect("stored entry should be found");
    assert_eq!(entry.outcome, Some(Outcome::Win));
    assert_eq!(entry.best_move, mv);
    assert_eq!(entry.depth, 7);
    assert!(entry.result_for(Outcome::Win).is_some());
    assert!(entry.result_for(Outcome::Loss).is_none());
}

#[test]
fn unsolved_bounds_do_not_overwrite_solved() {
    let mut tt = TranspositionTable::with_mb(1);
    let key = 12345u64;
    let mv = Move::make_move(
        atomic_movegen::types::Square::A1,
        atomic_movegen::types::Square::A2,
    );

    tt.store(
        key,
        mv,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        crate::zobrist::INF,
        7,
        u32::MAX,
    );
    tt.store(key, Move::NONE, u8::MAX, 100, None, 1, 1, 0, 0);

    let entry = tt.probe(key).expect("entry should still exist");
    assert_eq!(entry.outcome, Some(Outcome::Win));
    assert_eq!(entry.best_move, mv);
}

#[test]
fn solved_result_overwrites_unsolved_bounds() {
    let mut tt = TranspositionTable::with_mb(1);
    let key = 12345u64;
    let mv = Move::make_move(
        atomic_movegen::types::Square::A1,
        atomic_movegen::types::Square::A2,
    );

    tt.store(key, Move::NONE, u8::MAX, 0, None, 1, 1, 0, 0);
    tt.store(
        key,
        mv,
        u8::MAX,
        0,
        Some(Outcome::Loss),
        crate::zobrist::INF,
        0,
        12,
        u32::MAX,
    );

    let entry = tt.probe(key).expect("entry should exist");
    assert_eq!(entry.outcome, Some(Outcome::Loss));
    assert_eq!(entry.best_move, mv);
    assert_eq!(entry.depth, 12);
}

#[test]
fn new_generation_marks_old_entries_stale() {
    let mut tt = TranspositionTable::with_mb(1);
    let key = 123u64;

    tt.store(
        key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key).is_some());

    tt.new_generation();
    assert!(tt.probe(key).is_none());

    // Storing the same key in the new generation makes it visible again.
    tt.store(
        key,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key).is_some());
}

#[test]
fn new_generation_prefers_stale_slot() {
    let mut tt = TranspositionTable::with_mb(1);
    let key1 = 1u64;
    // Force two slots in the same bucket by using keys that differ by a
    // multiple of the bucket count.
    let bucket_count = tt.bucket_count() as u64;
    let key2 = key1 + bucket_count;

    tt.store(
        key1,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    tt.store(
        key2,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Win),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key1).is_some());
    assert!(tt.probe(key2).is_some());

    tt.new_generation();

    // A third key landing in the same bucket should overwrite a stale slot
    // instead of evicting a single live slot and losing the other.
    let key3 = key1 + 2 * bucket_count;
    tt.store(
        key3,
        Move::NONE,
        u8::MAX,
        0,
        Some(Outcome::Draw),
        0,
        0,
        0,
        u32::MAX,
    );
    assert!(tt.probe(key3).is_some());
    assert!(
        tt.probe(key1).is_none() || tt.probe(key2).is_none(),
        "new entry should overwrite a stale slot, not both live slots"
    );
}
