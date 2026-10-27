# Initiative: `performance` — product latency and throughput follow-up

## Status

**Active (opened 2026-10-27).** The source is
[`docs/performance/performance_report1.md`](../../performance/performance_report1.md)
(commit `b77255e`), a profile of the product solver on the reference
container. The report separates *product-visible latency* from raw search
throughput; this initiative owns the former and feeds the latter back to
`lean`/`dfpn` when it is genuinely a throughput lever.

Plan numbering is sequential within this directory. The report's ranked
opportunities (§10) are the backlog; the item ↔ plan mapping is kept here.

## Goal

Turn the performance report's measured opportunities into product changes
where the evidence and the pre-registered gate justify them, without
changing game-theoretic outcomes or the documented search contracts.

Priorities follow `AGENTS.md`: correctness, performance, memory,
maintainability, testability, consistency.

## Backlog (from report §10, ranked)

| # | Item | Report evidence | Plan | Status |
|---|------|-----------------|------|--------|
| 1 | Preflight cost gate / cheap-search probe | Easy 3-man KQ wins are ~60–75× slower with preflight (`~0.75 s / 5.8M evals` vs `~0.01 s / 229–337 evals`); draw-class preflight is a large win | `plan1.md` → `report1.md` | **done (GO, closed 2026-10-27)**: bounded DF-PN probe (2,000-eval cap, scratch TT) before the pre-phase; easy wins 0.75–0.80 s → 0.010–0.011 s (evals 5.83M → 2–586), draws/hard wins +2,000–2,019 evals (≤ 0.087%) and ≤ +2.5% wall, TT isolation + quick suite 59/59 eval-identical |
| 2 | TT pre-probe before make/unmake | 13–38% of child evals already find a usable TT entry; estimated 3–8% throughput | — | open (throughput; may move to `lean`) |
| 3 | Incremental repetition-context hash | Bit-identical, ~2–3% throughput | — | open (throughput; may move to `lean`) |
| 4 | Path Bloom negative filter | ~1–3%, optional second-order | — | open |
| 5 | Lazy/zero-init TT allocation | ~6 ms and ~220 MB RSS on short solves (report §4/§6.1); plan-session probe on the current container: **~100 ms / ~230 MB** per process, easy KQ 106 → 2.8 ms / 8 MB with a lazy prototype at a measured **+1–5% deep-run wall** cost | `plan2.md` → `report2.md` | **done (ADOPT, 2026-10-27)**: `alloc_zeroed` bucket array (`search/tt/alloc.rs`), label = reservation bound; mate-in-1 104.6 → 1.2 ms / 229 → 16 MB (87×), easy KQ 1.7 ms / 16 MB, `--tt-size` sweep flat 1–512 MB; closure class −8.5…−11.1% wall; deep +2.7–3.6% (`max` 1.036 ≤ 1.06, D-f not triggered); dec10 RSS 227 → 178 MB; all cells child-eval identical, golden unregoldened, quick 59/59, miri clean. One bar-3 miss recorded (ladder root −8.5%: fixed 86 ms of a 1.01 s solve) |
| 6 | Faster repetition-cache hasher | <1% | — | deferred |
| 7 | Unstable move sort | ~1%, search-visible tie order; full gate required | — | not recommended on current evidence |
| 8 | Huge pages / `MADV_HUGEPAGE` | not demonstrated; a plan2 plan-session `/tmp` probe measured **no effect** on the current container (THP `madvise`-only, no collapse on a 224 MiB region) for both the eager and the lazy allocation | — | deferred (negative evidence recorded in `plan2.md`; `report2.md` §Findings 5: with the lazy table now shipped, THP is the configuration most likely to hurt) |
| 9 | `atomic-movegen` hot paths | ~50% of CPU; upstream | — | upstream (`movegen` initiative) |
| 10 | Parallel/portfolio | m20 is an algorithmic/cost issue | — | handled by `parallel` |

## Conventions

- **Product latency and throughput are separate verdicts.** A change that
  improves a short-solve latency metric does not need a `lean` drift gate
  unless it also changes search trajectories; a change that alters the
  DF-PN trajectory needs the relevant gate in its own plan.
- **Preflight is a claim surface, not a micro-optimization.** Any change to
  when the pre-phase runs must preserve the soundness contract in
  `src/search/preflight/mod.rs` (no production-TT interaction, no wall-clock
  use, no `ProofEvent`s, budget accounting, replay-verified decisive claims)
  or explicitly document and test the exception.
- **Measure the class, not one draw.** 3-man latency claims use a corpus with
  easy wins, hard wins, draws, and terminal roots; report the
  `child_evals` (deterministic) and wall/RSS.
- **No new CLI knob unless the plan says so.** The existing
  `--no-preflight` remains the operator escape hatch.

## Item ↔ plan mapping

| item | plan | report | status |
|---|---|---|---|
| #1 preflight cost gate / cheap-search probe | `plan1.md` | `report1.md` | done — GO |
| #5 lazy/zero-init TT allocation | `plan2.md` | `report2.md` | done — ADOPT (bar 3 partial, documented) |

## Cross-references

- `docs/plans/lean/` — throughput/wall-time levers with the bit-identical
  drift protocol; items 2/3/4/5 may be picked up there.
- `docs/plans/dfpn/` — search-semantics work; the cheap probe changes *when*
  preflight runs, not DF-PN thresholds.
- `src/search/preflight/mod.rs` — the normative pre-phase soundness
  contract.
