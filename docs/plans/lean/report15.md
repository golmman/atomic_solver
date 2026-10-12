# Lean Report 15 — #20 honest TT sizing + #16(second) default bump 128 → 256 MB

Executed 2026-10-11 per `plan15.md`. The plan's falsifiable core claim —
the new default table is **byte-identical** to the old one, so every
default-mode artifact passes unchanged — held: all five pre-registered
identity checks passed with no regoldening and no re-baselining.
`cargo fmt --check` and `cargo clippy --all-targets` clean.

## What shipped

1. **Honest `with_mb` (task 1)** — `src/search/tt/table.rs`: the bucket
   count is now rounded *down* to a power of two (`raw =
   bytes / 224 B`, `buckets = raw.next_power_of_two() >> 1` unless
   already a power of two), so `buckets · 224 ≤ mb · 2²⁰` — the
   allocation never exceeds the label. The old `.max(32)` entry floor is
   replaced by a ≥ 1-bucket floor. Module-header sizing contract and the
   `with_mb` doc rewritten (mask indexing requires power-of-two buckets;
   exact-capacity modulo rejected per the pre-registered decision;
   56·2^k MB labels divide exactly and are unchanged).
2. **Unit tests (task 2)** — `src/search/tt/tests.rs`: the plan14 pin
   `with_mb_128_yields_quarter_the_buckets_of_the_old_layout` rewritten
   (`with_mb_128_yields_half_the_old_default_buckets`, 2^19 buckets /
   112 MiB); new tests: `with_mb_256_equals_the_old_default_table`
   (2^20 buckets / 224 MiB — the parity property), `with_mb_exact_
   divisors_are_unchanged_by_rounding` (56/112/224 MB → 2^18/2^19/2^20),
   `with_mb_1_yields_4096_buckets` (the module's generic small-table
   pin), and `with_mb_never_allocates_more_than_the_label` (honesty
   sweep over mb 1..=64 ∪ {128, 224, 256, 512, 1024}). All fast-tier.
3. **CLI default bump (task 3)** — `src/cli.rs` default `tt_size`
   128 → 256 (comment records that this is label-honesty bookkeeping,
   not a capacity change; #16's history kept); `defaults_match_
   documentation` updated. `src/main.rs` module header + help text:
   "(default: 256; never allocates more than requested)".
   `tests/test_cli.rs` doc comment updated.
4. **Benchmark lockstep (task 4)** — `examples/benchmark.rs` default
   `tt_size` 128 → 256 with a comment pinning it to the shipped CLI
   default (the evaluator must measure the shipped config; tuning
   continuity is bit-exact because old-128 == new-256). Spec example
   value `"tt_size": 256` in `docs/spec/optimizer_interface.md` (the
   option-table wording needed no change — verified: it pins nothing
   numeric).
5. **Doc sweep (task 6)** — `examples/portfolio.rs` RAM note: "4 × 256 MB
   labels = 4 × 224 MiB = 896 MiB of actual allocation";
   `tests/portfolio_smoke.rs` resource note updated to match;
   `docs/proofdb_pipeline.md` `--tt-mb` bullet: default unchanged at
   128, plus "since lean plan15 size labels are exact upper bounds —
   128 MB now means ≤ 112 MiB"; `src/tt_snapshot/mod.rs` header clause
   (`tt_size_mb` records the label; labels are exact allocation upper
   bounds since lean plan15); `tests/test_trajectory_golden.rs` header
   gains a re-validated-unchanged note.

## Identity validation (task 5, pre-registered; all green)

| # | Check | Result |
|---|-------|--------|
| 1 | Fast gate `cargo test --release` | 33 suites ok, 0 failed (incl. `portfolio_smoke`) |
| 2 | Golden unchanged: `test_trajectory_golden -- --include-ignored` | passes **without regenerating** the fixture |
| 3 | `portfolio_smoke` dec10 anchor | both tests pass; 3,901,847 evals exact |
| 4 | Quick-suite eval identity vs `measurements/plan14/quick_arma.json` (`benchmark --suite quick --json --first-outcome --timeout 3`, evals/outcomes/nodes/pv_len compared; wall and the `tt_size` echo excluded — the echo now reads 256) | **59/59 identical, 0 diffs** |
| 5 | `cargo fmt --check`, `cargo clippy --all-targets` | clean |

The falsification protocol was not needed.

## Size-mapping table (shipped)

| `--tt-size` label | new table (round down) | old table (round up) |
|---|---|---|
| 64 MB | 2^18 buckets, 56 MiB | 2^19 buckets, 112 MiB |
| 128 MB (old default) | 2^19 buckets, 112 MiB | 2^20 buckets, 224 MiB |
| **256 MB (new default)** | **2^20 buckets, 224 MiB = old default** | 2^21 buckets, 448 MiB |
| 56·2^k MB (56/112/224/448…) | exact, unchanged | exact |

**Explicit-size user guidance:** anyone passing `--tt-size 128`
explicitly now gets half the table they used to (112 MiB). That is the
intended honesty — scripts wanting the old default table should move to
`--tt-size 256`. No compat shim was added (pre-registered decision).

**Measurement continuity:** plan13/plan14 stress cells quoted at
"128 MB" correspond to the **new label 256** (identical table, so the
numbers stay comparable verbatim). Future mechanism reads must pick
label 256 for those settings.

**No capacity was added anywhere.** The default table is byte-identical
to the pre-change default; the honest statement is that this plan
changed labels and allocation accounting only. Report14's stress-class
capacity opportunity therefore stays open and is now a
*larger-explicit-size* question (e.g. label 512 → 448 MiB instead of the
old 512 → 896 MiB), an owner's call. Arm B (layered TT) stays **dormant**
with report14's sizing as its input.

## Tools and examples used

- `cargo test --release` (fast gate), `cargo test --release --test
  test_trajectory_golden -- --include-ignored` (golden), `cargo run
  --release --example benchmark -- --suite quick --json
  --first-outcome --timeout 3` (identity run, transcript under
  `/tmp`, not committed — regenerable), a short Python comparison
  script (regenerable one-liner), `cargo fmt --check` /
  `cargo clippy --all-targets`.

## Problems encountered

- None material. One grep sweep after the doc sweep caught two test-file
  RAM notes still quoting the old default (4 × 128 MB = 512 MB in
  `tests/portfolio_smoke.rs`; the golden test header lacked a plan15
  note) — both fixed; the second gate re-run stayed green.

## Unresolved parts / missing tests

- No new `#[ignore]`d tests (per plan): the golden run is the existing
  slow test, re-run explicitly. The honesty sweep is fast-tier only;
  labels > 1 GiB are not unit-tested (allocation cost), only covered by
  the sweep's formula property up to 1024.
- The snapshot `tt_size_mb` header field semantics ("the label the
  search ran with") is now documented but not asserted anywhere new; the
  existing round-trip test pins it by explicit value.
- `examples/find_winning_child` (128) and the other example binaries
  just get new capacities under the same labels, as planned; none were
  re-measured (no fixture pins them).

## Next steps

1. Owner decision on the now-explicitly-sized capacity lever: whether
   the stress-class capacity opportunity is worth a *larger explicit
   size* (e.g. `--tt-size 512` → 448 MiB) re-measurement on the m19/m20
   stress cells, or whether #21 closes with report14 + this report as
   the record.
2. Arm B (layered TT) stays dormant; its trigger and sizing input are
   report14's next-steps section.

SESSION COMPLETE
- Deliverables: honest round-down `with_mb` shipped (`src/search/tt/
  table.rs` + 5 unit tests); default `--tt-size` 128 → 256 (`src/cli.rs`,
  `src/main.rs`, `tests/test_cli.rs`); benchmark default bumped in
  lockstep + spec example value (`examples/benchmark.rs`,
  `docs/spec/optimizer_interface.md`); doc sweep (`examples/portfolio.rs`,
  `tests/portfolio_smoke.rs`, `tests/test_trajectory_golden.rs`,
  `docs/proofdb_pipeline.md`, `src/tt_snapshot/mod.rs`); backlog #20/#16/
  #21 rows + history updated; `docs/plans/README.md` lean row updated;
  `report15.md` written. Gate: `make test` green, fmt/clippy clean;
  identity checks 1–5 all green (golden unchanged, dec10 anchor exact,
  quick suite 59/59 eval-identical).
Follow-up options:
  1. **Owner decision on the capacity family** — close #21 on the
     report14+report15 record, or plan a larger-explicit-size stress
     re-probe (label 512 → 448 MiB on the m19/m20 cells at gate grade);
     kickoff prompt: "Read `docs/plans/lean/report14.md` next steps and
     `report15.md`'s capacity statement; decide #21's final disposition
     (close vs. label-512 stress re-probe) and write the plan or the
     closure accordingly."
  2. **Leave the lean initiative as-is** (backlog otherwise empty of
     performance levers; #7 lazy/staged child evaluation remains the
     last open item); reason: no further lever has a measured GO
     indication — defer until a new diagnostic suggests one.
