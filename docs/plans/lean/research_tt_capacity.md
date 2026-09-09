# Research: TT capacity vs. work on hard positions

Research round requested after the #12/#14 spikes (2026-09-09). Trigger:
while spiking #12 we noticed m22_white first-outcome did ~48% less work at
`--tt-size 512` than at the 64 MB default. This note sizes that effect,
rules out entry-slimming as the fix, and frames the follow-up decision.

## Method

- Positions: `m22_white` (dominant hard case) and the quick suite (59
  cases) as the shallow-control group.
- `m22_white` first-outcome, one deterministic run per size, via a
  temporary probe example (`Search::solve`, `--first-outcome` semantics);
  `child_evals` is exactly reproducible per size. Probe instrumentation
  was removed after measuring.
- Quick suite: `benchmark --suite quick --json --first-outcome --runs 1
  --tt-size {64,128}`.

## Results — m22_white first-outcome

| TT size | wall | nodes | child_evals | live entries at end |
|---|---|---|---|---|
| 64 MB (default) | 5.26 s | 2.45 M | **37.5 M** | 1.02 M (table full) |
| 128 MB | **2.57 s** | 0.86 M | **14.2 M** | 0.70 M |
| 256 MB | 3.26 s | 1.10 M | 17.6 M | 0.85 M |
| 512 MB | 4.04 s | 1.15 M | 19.4 M | 0.96 M |
| 1024 MB | 4.03 s | 1.01 M | 16.8 M | 0.84 M |

- **The default 64 MB table is capacity-starved on m22-class searches**:
  ~1M live entries for a 2.4M-node search, ~60% churn. Doubling to
  128 MB cuts work 2.6× and wall 2.0×.
- **Above ~128 MB there is no monotone trend**: work varies 14–19M
  chaotically (DF-PN is trajectory-sensitive to TT contents — different
  replacement decisions reorder the search). Treat any single-size number
  above 128 MB as one draw from a distribution.
- **Wall time does not follow `child_evals` at large sizes**: nps falls
  from 434k (64 MB) to 238k (1024 MB) as the sparse table probes cold
  cache lines. The wall optimum on this container is ~128 MB.

## Results — quick suite control

| TT size | total child_evals | not ok |
|---|---|---|
| 64 MB | 38,714,551 (matches plan1–3 artifacts) | 0 |
| 128 MB | 39,496,274 (+2.0%) | 0 |

The quick suite is TT-size-insensitive: every case is shallow enough that
64 MB never evicts live work (per-case deltas are within ±1% except
`dec10`, −29%, and `m23_black`, −1%... dec10's "win" is a regression from
the m22-class chaos, not a capacity effect). The capacity penalty is a
**deep-search phenomenon** and shows up in the move-order suite (the hard
tier), which is exactly where wall time matters most (output priority 1
in AGENTS.md).

## Why not entry slimming

Current `TtEntry` is 56 B: `key` u64, `valid` bool, `generation` u32,
`best_move` 2 B, `best_child` u8, `work` u64, `outcome` 1 B, `pn`/`dn`
u64, `depth`/`remaining_depth` u32.

- `pn`/`dn` cannot shrink: `zobrist::INF = 1 << 60` and the solver's
  saturation logic relies on the wide sentinel. Changing INF semantics is
  a solver-wide refactor.
- `work: u64` → u32 (saturating) and folding `valid` into
  `generation == 0` saves ~8–16 B (56 → 40–48 B): +17–40% capacity. For
  m22 the table needs ≥ 2× capacity; slimming alone does not close the
  gap, and above the capacity knee the chaotic regime makes small
  capacity changes unpredictable in sign.
- A 4-slot bucket at the same MB doubles capacity but widens the probe
  scan; untested, same chaos caveat.

Conclusion: the cheap, predictable fix is **capacity itself** (a bigger
default), not layout surgery.

## Consequences and constraints

1. This is a **search-behavior-changing** lever: the m22 trajectory and
   the quick-suite `dec10`-class deltas mean the bit-identical drift
   protocol cannot pass. Per the initiative's non-goals, such a plan must
   validate against the move-order benchmark suite (the tier designed for
   ordering/behavior changes) and re-baseline the m22 goldens
   (`tests/fixtures/m22_default_stdout_golden.txt` and the chunk
   trajectory artifacts).
2. The optimizer interface contract (`docs/spec/optimizer_interface.md`)
   exposes `--tt-size`; the spec's default must be updated in the same
   plan if the default changes (standalone-spec rule applies).
3. Memory is a stated quality attribute: 128 MB default doubles the
   table. The evidence says the trade is strongly favorable for hard
   positions (the stated output priority) and neutral for shallow ones.

## Options for a follow-up plan

- **(a) Bump the default `--tt-size` 64 → 128 MB.** One-line change plus
  spec/goldens re-baseline. Expected: ~2× wall on m22-class hard cases,
  ±2% on the quick suite.
- **(b) Keep 64 MB default, document `--tt-size 128` guidance** for hard
  solves. Zero behavior risk, no win at default settings.
- **(c) Size-adaptive default** (e.g. scale TT with available RAM or with
  a `--hard` preset). More surface, no evidence yet beyond one position.

Recommendation: (a), validated on the full move-order suite (all 20
cases, first-outcome and default mode) before adopting, since the m22
single-position evidence is one draw from the chaotic regime.

## Related observations (not sized here)

- The 2-slot work-aware replacement (`insert_new` score = live, solved,
  work, generation) is already ultimattt-style; no policy change is
  implied by the capacity data.
- `new_generation()` is never called in production code (test-only);
  the table persists across chunks. If future plans add per-round
  invalidation, the capacity knee will move and this study must be
  repeated.
- Entry slimming beyond ~40 B is blocked by `INF = 1 << 60` (u64 pn/dn).
