# Report 4 — breadth-first PNS harvester: selection state in a sidecar, first batch

Executes `plan4.md` (the plan3 pivot). All deliverables landed; all
pre-registered gates **pass**. The measured yield is **0 facts at 300.0M
child-evals — exactly the pre-registered expectation** (plan3 P0 had spent
the identical per-node budget over the same open class at 0; only the visit
order changed). The batch's real deliverable is the **ledger baseline**: a
1,443-record sidecar (75 censored nodes at pass 1 + 1,368 freshly exposed
undecided children) that makes the exposed frontier durable across
sessions. The live queue behaved as pre-registered: a strict breadth-first
descent (plies 0 → 1 → 2 fully censored at 4M each) — the first ladder rung
never fired in-session, so escalation is a cross-session dynamic.

## Deliverables

- **D1** PNS machinery, `examples/proofdb/` (+ tests via `tests/proofdb.rs`;
  split for the 10 KB convention — plan4 code: `ledger.rs`,
  `pns.rs`, `pns/build.rs`, `pns/numbers.rs`, `pns/selector.rs`, tests in
  `pns/tests.rs`, `pns/selector/tests.rs`, `ledger/tests.rs`; all ≤ 10 KB):
  - `ledger.rs` — the sidecar work ledger (`data/proofdb_work.json`):
    one record per known-open frontier node (`work_done`,
    `passes_failed`), deterministic sorted bytes, temp-file+rename saves,
    missing file = fresh init, malformed/duplicate records abort.
  - `pns.rs` + `pns/build.rs` — the extended tree (DB rows ∪ ledger
    records), the **lineage gate** (decided paths dropped at load, illegal
    paths abort the session), the job set with per-reason exclusions
    (sibling-skip decision 9, decided-row decision 10), census.
  - `pns/numbers.rs` — structural pn/dn over the extended tree (unvisited
    = 1 via movegen, proven = 0/∞ by root parity, min/sum by OR/AND,
    known-open children flat `1 + passes_failed`), saturating arithmetic
    (finite sums never produce a false ∞), effective numbers
    `max(structural, 1 + passes_failed)`.
  - `pns/selector.rs` — the live priority queue `(effective number, ply,
    path)`: pop, censor-time bump + frontier exposure (fresh records for
    every unexpanded legal child, saved atomically per visit), decided
    bookkeeping, geometric ladder `2^(k-1) × 4M`.
- **D2** CLI (`proofdb_harvest`): `--policy breadth-pns` (new default;
  the plan3 gradients stay selectable), `--ledger <path>` (default
  `data/proofdb_work.json`), census triple `pass`/`number`/`work_before`
  on the `job:` lines (legacy policies emit without it), new session-start
  census line (`pns: …`), `--max-total-evals` as the session cap.
  No changes to session/export/manifest machinery (JobRecord gained three
  optional census fields); no `src/` changes.
- **D3** working layer at `data/` (gitignored): `.gitignore` entry
  (supersedes the nn-era `data/corpus|oracle` entries), `data/proofdb.db`
  built from the standing shard set — **byte-identical to plan3's committed
  `proofdb_grown2.db`** (sha256 `670e19e3…`) — and the empty ledger init.
- **D4** `measurements/plan4/`: `census_breadth-pns.json` (75 job records +
  summary + exclusion census), `ledger_snapshot.json` (the post-batch
  escalation state), `assemble_census.py` (driver + H1/H5 checks),
  `env.json`, `README.md` (provenance + command table + the measured
  result). No grown-DB copy needed: 0 new shards, so the post-batch DB is
  byte-identical to the input (= plan3's committed artifact). Raw
  transcripts not committed (measurement conventions).
- **D5** this report.

## The batch (root = startpos; decision 5 budgets, no mid-run tuning)

| metric | value |
| --- | --- |
| session-start census | 75 open rows → 26 excluded (proven-ancestor), **49 jobs** (all number 1); ledger empty |
| jobs run | 75 — 9 C1 (open rows) + 66 L (fresh ledger children) |
| outcomes | 75 censored (`BudgetExhausted`), 0 decisive |
| evals | 300,001,024 child-evals (cap 300M, between-jobs check) |
| wall time | 65.1 s (≈ 4.6M evals/s) |
| post-batch ledger | 1,443 records: 75 censored (`passes_failed` 1, ≈ 4.0M `work_done` each) + 1,368 exposed (`passes_failed` 0) |
| visited plies | 0 (1), 1 (20), 2 (54) — **ply sequence strictly non-decreasing** |

**The breadth-first gradient verified:** the `(number, ply, path)` ordering
produced an exact ply-layer sweep — root, the full ply-1 layer (7 open rows
+ 13 freshly exposed), then the ply-2 layer (54 jobs) — and the cap fired
before the ply-3 layer began. The 40 initial jobs at plies 3–31 are still
queued (undecided rows, re-derived next session); the 1,368 exposed
children are the durable frontier. The first ladder rung (8M revisits)
never fired in-session: with 0 facts, every censor exposes fresh number-1
children, so the number-1 pool never drains within a session. This is the
pre-registered layer discipline working verbatim ("deeper expansion begins
only once the current depth is explored or in flight") — and it makes the
§8 budget arithmetic ("number-1 pool × 4M ≈ 164M, headroom for first ladder
rungs") descriptive only of a pool-draining regime that a factless BFS
never reaches.

## Gate verdicts

| gate | verdict | notes |
| --- | --- | --- |
| H1 job/shard integrity | **pass** | 0 new shards; the merger's 197/197 replay-validations + manifest cross-checks + hash fidelity re-ran clean (conflicts 0, skeleton re-validations 197); lineage gate ran (0 dropped, 0 illegal — fresh ledger); exclusion census reported per reason; every job path is one of the 49 non-excluded rows or a fresh ledger child (checked in `assemble_census.py`) |
| H2 merge-after-batch determinism | **pass** | two full merger runs over the unchanged manifest → DB `670e19e3…` and dump `e3060d14…` byte-identical |
| H3 no-regression vs input DB | **pass** | 0 new shards → the post-batch DB is byte-identical to the input (no outcome flip, no bound increase, nothing proven→open — vacuously and bytewise) |
| H4 new-fact spot-checks | **pass (vacuous)** | no new facts, nothing to spot-check |
| H5 policy determinism | **pass** | batch re-run from the same input DB + same (initial) ledger + fresh TT (throwaway manifest/shard-dir): 75/75 job records identical on every field except wall time, identical job sequence, and **byte-identical post-batch ledger**; doubles as verification that the module split was pure code motion |
| H6 hygiene | **pass** | `make test` green (all test binaries, incl. the new ledger/pns/selector tests — 32 in the proofdb target); `cargo clippy --release --all-targets` 0 warnings; `cargo fmt --check` clean; no `src/` changes; all new/edited example files ≤ 10 KB; `git status` confirms `data/` ignored |

## Findings

1. **The plan §1 census was a no-movegen approximation.** §1 pre-registered
   "26 behind a proven win, 7 implied wins, 1 implied loss, 41 jobs";
   measured 26 / 0 / 0 / **49**. The 7 "implied wins" are AND rows (the
   1.f3-type positions) whose 11–17 *stored* replies are all proven
   root-wins but which retain 2–9 unvisited replies — under §2's normative
   rule (unvisited = 1 via movegen) their pn is > 0, i.e. genuinely
   undecided, and decision 10 ("harvesting would re-derive an
   already-implied fact") does not apply. §2's rule is what the
   implementation follows; recorded in the census JSON and README. The 8
   extra jobs are exactly those rows.
2. **The live queue turns a factless session into pure BFS.** With 0 facts
   the number-1 pool never drains, so within a session the ladder is
   unreachable and selection degenerates to a ply-layer sweep. The plan's
   layer discipline intends exactly this, but its consequence is worth
   stating: *the first session after a yieldless batch re-walks the exposed
   frontier breadth-first at the base budget, not the censored nodes at
   their doubled rungs.* Ladder rungs fire only once a layer starts
   producing facts (proven children shrink the pool) or a layer's children
   are all already recorded. Next session's expectation: plies 0–2 are all
   censored (pass 1), their ~1,400 exposed children at number 1 outrank
   them, and the session will spend its cap on ply-3+ layers — unless the
   ledger's work-proportional or layer-drain selection is revisited (below).
3. **Exposure is cheap, breadth explodes.** 75 visits exposed 1,368
   children (≈ 27 per node — wide atomic positions); the ledger file is
   90 KB. A second session would census 49 + 1,368 = 1,417 number-1 jobs;
   at 4M each the 300M cap covers ~75 of them, again ~2 ply layers. The
   BFS layer cost grows ~27× per ply, so the current policy explores depth,
   not lines — the opposite of "most-proving lines first" once the cheap
   head is gone. This is the measured data point for the pivot's premise:
   the frontier is quiet ≥ 4M per node, and breadth-first PNS with
   censor-bump numbers cannot prioritize within the number-1 pool. A
   next-generation selector would need work-aware numbers (the
   pre-registered non-goal) or a per-layer budget split to make progress
   past ply 3–4.
4. **The refactor gate is free.** H5's byte-level ledger comparison caught
   nothing because nothing changed — but the same replay verified the
   mid-session module split (`pns.rs` → `pns/{build,numbers,selector}`,
   forced by the 10 KB convention) as pure code motion at zero extra cost.

## Problems encountered

- The plan §1 census discrepancy (finding 1) surfaced as a failed
  expectation on the first census probe run (throwaway ledger); resolved by
  treating §2's movegen-based rule as normative — it is the pre-registered
  policy text and the sound reading. No code change was involved.
- File-size convention: `pns.rs` landed at 20.6 KB in the first draft and
  was split into `pns/{build,numbers,selector}` (pure code motion, verified
  by the H5 replay); the test file was split the same way
  (`pns/selector/tests.rs`).
- No tool defects found in the batch run itself: 75/75 jobs ran clean, the
  ledger was rewritten 75 times (once per censored visit), and the replay
  reproduced everything byte-identically.

## Unresolved / next steps

1. **Selection-within-layer (new, follows from finding 3):** the measured
   BFS dynamic spends the whole session on one or two ply layers. Before
   the next large batch, decide whether number-1 ties should break by
   work-aware or shallow-subtree-aware keys — otherwise the ladder and the
   "most-proving lines first" premise stay theoretical. Small change to
   `pns/selector.rs`'s ordering key; needs a plan (the current rule is
   pre-registered, so changing it is a plan-level decision).
2. **Website handoff (item 5)** remains the highest-leverage step (34,980
   proven positions, spec'd schema); the PNS ladder mainly serves proof
   depth, not coverage.
3. **Independent harvesters (item 6):** the ledger now holds exactly the
   pick-up state a per-worker partition needs (decision-path records);
   the sequential measurement is done, so the partition design can start.
4. **Upward propagation pass:** the 7 near-implied AND rows (finding 1)
   would flip to proven as soon as their unvisited replies are handled —
   a future propagation pass over the exposed frontier is the natural
   consumer of the ledger.
5. DTM-upgrade pass (item 3), cross-session TT seeding: unchanged, deferred.

## Tools used

- `cargo build/test/clippy/fmt` (release), `python3` (census assembly +
  read-only DB probes via sqlite3), the merger/harvest binaries. No new
  external tooling. All probes ran against throwaway copies in `/tmp`; the
  standing shard set, manifest, and `data/` ledger were only touched by
  the real batch.
