# Lean Plan 15 — #20 + #16(second): memory-honest TT sizing, default bump 128 → 256 MB

Initiative: `lean`. Executes backlog item **#20** (TT allocation sizing: the
owner decision is *round down*) and amends **#16** (second default bump,
`--tt-size` 128 → 256 MB); records the **#21** capacity-family disposition
from `report14.md`'s next steps (owner decision this session: lever A +
default bump, chosen over arm B / closing). Self-contained. **This is not a
performance lever**: the new default table is byte-identical to the old one
(probed, see Context 2), so every default-mode artifact should pass
**unchanged** — that identity is the plan's falsifiable core claim, and the
validation protocol (task 5) is built around it. No gate v1.0 rollout, no
regoldening (contrast plan14).

## Context the executor must know

1. **Current sizing** (`src/search/tt/table.rs`, `with_mb`):
   `entries = (mb·2²⁰/56).next_power_of_two().max(32)`, `buckets =
   max(entries,4)/4`. Bucket index is `key & mask` with `mask = buckets-1`
   (salt 0, shipped path) — the mask **requires a power-of-two bucket
   count**. The round-up is why `--tt-size 128` allocates 224 MiB (1.75×
   the label; backlog #20). Entries are 56 B, buckets 4×56 = 224 B
   (plan14's 4-slot layout).
2. **The parity property (owner decision, verified by probe).** Rounding
   *down* to a power-of-two bucket count and bumping the default label
   128 → 256 MB yields **exactly the old default table**:

   | label | old table (round up) | new table (round down) |
   |---|---|---|
   | 64 MB | 2¹⁹ buckets, 112 MiB | 2¹⁸ buckets, 56 MiB |
   | 128 MB (old default) | 2²⁰ buckets, 224 MiB | 2¹⁹ buckets, 112 MiB |
   | **256 MB (new default)** | 2²¹ buckets, 448 MiB | **2²⁰ buckets, 224 MiB = old default** |
   | 56·2^k MB (56/112/224/448…) | exact | **unchanged** |

   `with_mb(256)` → 2²⁰ buckets / 2²² entries / 224 MiB — identical bucket
   count, entry count, and allocation to the shipped 128 MB table. Default
   stdout has no tt-size echo (golden fixture is `outcome`/`pv`/`pv_status`
   lines only), so default-mode artifacts must be byte-identical. Sizes
   where `mb·2²⁰/56` is exactly a power of two (56·2^k MB) keep identical
   tables under both formulas — useful test pins.
3. **Snapshots are unaffected by design.** `tt_snapshot` records (full
   64-bit) keys, never bucket indices; entries store full keys and `probe`
   verifies them, so a snapshot restored under the new formula recomputes
   the mapping consistently. No format change, no version bump. The
   `tt_size_mb` header field keeps meaning "the label the search ran with".
   `examples/reconstruct_pt.rs` restores via
   `opts.tt_size.unwrap_or(header.tt_size_mb)` → `with_mb` — internally
   consistent under either formula.
4. **Probed consumer inventory** (what the change touches):
   - `src/cli.rs:57` default `tt_size: 128`; help/default text in
     `src/main.rs` (module header line ~15 "Defaults to 128.", help lines
     ~105–106); `src/cli.rs` test asserting the default (line ~251).
   - `src/search/tt/tests.rs`: `with_mb_128_yields_quarter_the_buckets_of_
     the_old_layout` (line ~106) pins the old formula and must be
     rewritten; all other tests use `with_mb(1)` as a generic small table
     (new: 4,096 buckets vs old 8,192 — no count assertions there).
   - `examples/benchmark.rs:109` has its **own** default `tt_size = 128`
     (the external optimizer's evaluator per `docs/spec/optimizer_
     interface.md`). It must bump to 256 in lockstep: the evaluator must
     measure the shipped default config, and old-128 == new-256 makes the
     tuning continuity bit-exact. The spec's example JSON shows
     `"tt_size": 128` (example value only; the option table pins nothing
     numeric). `tests/test_benchmark_json.rs` asserts field presence only.
   - `examples/portfolio.rs` passes **no** `--tt-size` by default → racers
     use the CLI default → with the bump they keep today's exact table;
     the dec10 anchor (3,901,847 evals, salt-insensitive) must hold
     unchanged. Its header RAM note ("4 × 128 MB = 512 MB at defaults",
     line ~22) needs the label/RAM rewording.
   - `docs/proofdb_pipeline.md` (~line 305): `proofdb_harvest --tt-mb`
     default 128 MB — **stays 128** (now an honest 112 MiB upper bound);
     add a half-sentence of wording, no default change.
   - `tests/test_cli.rs` (~line 314) doc comment "default `--tt-size` =
     128 MB"; the snapshot-bytes bound at ~347 (`≤ tt_size_mb · 1 MiB`)
     gains headroom, no change.
   - `src/tt_snapshot/mod.rs` header ("bounded by the TT RAM") — optional
     one-clause note that the label is now an exact allocation bound.
   - `src/search/dfpn/mod.rs:233` (`Search::new(tt_mb)`) and the other
     examples (`find_winning_child` 128, `chunk_growth`/`move_order_debug`/
     `twin_stats` 64, `play_and_solve`/`solve_depth_limited` 256,
     `campaign_*` via args) just get new capacities under the same labels;
     none pin byte-exact fixtures.
5. **Explicit-size users**: anyone passing `--tt-size 128` explicitly gets
   half the table they used to (112 MiB). That is the intended honesty;
   scripts wanting the old default table move to 256. This goes in the
   report as user guidance, not as a compat shim.
6. **Measurement continuity**: plan13/plan14 stress cells quoted at
   "128 MB" correspond to the new label **256**. The report must state
   this mapping so future mechanism reads pick comparable settings.

## Pre-registered decisions (do not re-litigate in-session)

- **Round-down-to-power-of-two buckets**, not exact-capacity modulo:
  honesty (never allocate more than the label) + the drift-free default
  outweigh using the last ~12.5% of the label; the mask indexing stays.
- **Benchmark bumps in lockstep** (256) for shipped-config parity.
- **Harvest `--tt-mb` stays 128.**
- **No gate rollout**: there is no lever adoption (report14's gate v1.0
  governs performance levers); default behavior is identical by
  construction, and identity checks (task 5) are the validation.

## The change

`src/search/tt/table.rs` — `with_mb` becomes:

```rust
#[must_use]
pub fn with_mb(mb: usize) -> Self {
    let bytes = mb.saturating_mul(1024 * 1024);
    let bucket_bytes = std::mem::size_of::<TtEntry>() * 4;
    let raw = (bytes / bucket_bytes).max(1);
    let buckets = if raw.is_power_of_two() {
        raw
    } else {
        raw.next_power_of_two() >> 1
    };
    Self {
        table: vec![[TtEntry::default(); 4]; buckets],
        mask: buckets - 1,
        current_generation: 1,
        salt: 0,
    }
}
```

Invariant: `buckets · 224 ≤ mb · 2²⁰` — the allocation never exceeds the
label. The old `.max(32)` entry floor is replaced by a ≥ 1-bucket floor.
Rewrite the module-header topology comment (plan14 sentence + plan15
sizing contract) and the `with_mb` doc.

## Tasks

1. **`with_mb` honest rounding** (code above) + module/`with_mb` doc
   rewrite in `src/search/tt/table.rs`.
2. **Unit tests** (`src/search/tt/tests.rs`): rewrite the 128-buckets test;
   new `with_mb` honesty-sweep test (for mb ∈ {1..=64} ∪ {128, 224, 256,
   512, 1024}: `bucket_count() · 224 ≤ mb · 2²⁰`); new parity-pin test
   (`with_mb(256)` → 2²⁰ buckets — the property task 5 relies on); pins
   for `with_mb(1)` → 4,096 buckets and an unchanged-size case (56 MB →
   2¹⁸ buckets).
3. **CLI default bump**: `src/cli.rs` default 128 → 256 + default-parse
   test; `src/main.rs` module header + help text (mention honest sizing,
   e.g. "the table never allocates more than requested");
   `tests/test_cli.rs` doc comment.
4. **Benchmark parity + spec**: `examples/benchmark.rs` default 128 → 256;
   update the example `"tt_size"` value in `docs/spec/optimizer_interface.md`
   (spec stays standalone; no wording beyond the example value needs to
   change — verify while there).
5. **Identity validation** (pre-registered; run in this order):
   1. `cargo test --release` (fast gate, incl. `portfolio_smoke`).
   2. Golden UNCHANGED: `cargo test --release --test test_trajectory_golden
      -- --include-ignored` must pass **without regenerating** the fixture.
   3. Anchor: `portfolio_smoke` dec10 assertions pass unchanged.
   4. Quick-suite eval identity: `benchmark --suite quick --json` vs
      `docs/plans/lean/measurements/plan14/quick_arma.json` — 59/59
      outcomes and per-case eval counts identical (wall may drift; compare
      evals only).
   5. `cargo fmt --check`, `cargo clippy --all-targets`.
   **Falsification handling:** if any identity check fails, the parity
   property is wrong — STOP and root-cause (first suspects: the formula's
   edge cases, or a hidden explicit-size dependency). Re-baselining is
   **not** pre-authorized by this plan; it requires an explicit addendum
   recording the root cause.
6. **Doc sweep**: `examples/portfolio.rs` RAM note (label 256 → "1 GB at
   defaults", actual 4 × 224 MiB = 896 MiB); `docs/proofdb_pipeline.md`
   half-sentence (labels are exact upper bounds since lean plan15; default
   unchanged); optional `tt_snapshot` header clause.
7. **Report + index**: write `report15.md` (must include: the size-mapping
   table; explicit-size user guidance "128 → 256 for the old default
   table"; the measurement-continuity mapping — plan13/14 "128 MB" cells =
   new label 256; the honest statement that **no capacity was added
   anywhere** — the default table is unchanged, so report14's stress-class
   capacity opportunity stays open and is now a *larger-explicit-size*
   question, e.g. label 512 → 448 MiB, owner's call; arm B dormant with
   report14's sizing); update `initiative.md` backlog rows (#20 → done;
   #16 → amend with the second bump; #21 → capacity-family disposition
   recorded, arm B dormant) and the `docs/plans/README.md` lean row.

## Non-goals

- No exact-capacity modulo indexing; no entry-format or snapshot-format
  change; no version bumps.
- No arm B / layered TT; no harvest default change; no stress-cell
  re-measurement; no `--tt-size` compat shim for scripts.

## Testing tier note

All new unit tests are fast-tier. The golden run (~20 s) is the existing
`#[ignore]`d test, invoked explicitly in task 5; do not add new
`#[ignore]`d tests for this change.
