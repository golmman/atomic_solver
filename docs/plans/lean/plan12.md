# Lean Plan 12 — #19 prefetch pre-pass keys via upstream `hash_after` (bit-identical)

Implements backlog lever **#19**: replace the plan11 prefetch pre-pass's
`do_move_with_scratch`/`undo_move_with_scratch` round-trip by the
non-mutating `Board::hash_after` / `Board::rule50_after` from
`atomic-movegen` 2.3.0 (upstream spec:
`docs/plans/movegen/plan_hash_after.md`). **One lever.**

## Preconditions (check first; stop and report if unmet)

1. Plan11 is landed (`report11.md` exists, pre-pass in
   `evaluate_all_children`).
2. `atomic-movegen` 2.3.0 is published with both functions and its
   equivalence property test. Write
   `docs/plans/movegen/report_update_2.3.0.md` (consumer-side notes, same
   pattern as `report_update_2.2.0.md`).
3. `report11.md` still measures the pre-pass at ≥ ~5% of wall (phase-0
   estimate ≈12%, spike V12 in `measurements/plan11_phase0/`). If plan11's
   report says otherwise, demote #19 instead of executing (the #17
   precedent).

Hard constraint: **bit-identical search** — the pre-pass only feeds
prefetch hints, so a wrong key would cost speed, never correctness; the
drift protocol still applies in full, and the key equality is
additionally tested directly (task 3).

## Design (settled)

1. `Cargo.toml`: `atomic-movegen = "2.3"`; `cargo update -p atomic-movegen`.
2. `src/position.rs`: add

   ```rust
   /// TT key (`hash()`) of the position after `m`, without playing it.
   /// Equal to `{ do_move(m); hash() }` (upstream `hash_after` /
   /// `rule50_after` contract).
   #[must_use]
   pub fn hash_after(&self, m: Move) -> u64 {
       self.board.hash_after(m) ^ zobrist::rule50_key(self.board.rule50_after(m))
   }
   ```

   `rule50_key` already clamps at 100, matching `refresh_zobrist`.
3. `src/search/dfpn/children.rs`: the pre-pass becomes
   `for i in 0..moves.len() { self.tt.prefetch(pos.hash_after(moves[i])); }`
   — the local pre-pass `StateInfo` and its doc sentence go away
   (Boy Scout: update the `ChildPrecompute` "Eval-scratch slots" docs
   back).

## Tasks

1. Baseline capture on the plan11 binary (same set as plan11 task 1:
   quick JSON, m22 FO/default stdout md5, shuffle-win FO stdout md5 +
   wall) → `measurements/plan12/`.
2. Implement design 1–3.
3. **Key-equality test** (`src/position.rs` tests): for a set of FENs
   (startpos, m22, shuffle-win, an en-passant position, a castling
   position, a promotion-capture position) and every legal move,
   `pos.hash_after(m)` equals `{ let mut p = pos.clone(); p.do_move(m); p.hash() }`;
   include one position with `rule50 = 99` and one with `rule50 = 100`
   (clamp).
4. Drift protocol (bit-identical vs task 1).
5. Wall A/B (sequential, interleaved): m22 default ×5, m22 FO ×5,
   shuffle-win FO ×2, quick suite ×1. Expected −5–10%; record the
   measured pre-pass residual share in a fresh leaf profile.
6. Gates: fmt, clippy, `make test`, `make test-full`.
7. Docs: lean `initiative.md` (backlog #19, profile, history), movegen
   `initiative.md` arc line.
8. **Write `report12.md`** (final task).
