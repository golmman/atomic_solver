# Report 7 — batch 3: the shipped-default control vs the ledger-union arm; the union mechanism ships

Executes `plan7.md`. All deliverables landed; all pre-registered gates
**pass**; the two-arm A/B completed from the standing post-plan6 state.
**The verdict: the §4.5 decision rule fires on all three conditions —
the ledger-union mechanism ships.** Arm 1 (control, the standing ledger
as-is) spent **24 re-censors = 96,000,242 evals = 32.0% of the 300M cap**
re-censoring ply-2 nodes that plan6 arm A had already censored (finding 4
made measurable, matching the plan's ≤ 96M estimate dead-on); arm 2 (the
5,950-record union base) had **0 re-censors** and spent the identical
session on genuinely new knowledge. Facts: **0 in both arms** (as
pre-registered — the sixth consecutive 0-fact 300M session), so the
**§4.6 closure rule fires: plan8 must change the yield outlook**. The
standing ledger advanced to arm 2's post-run state (sha `0d0a86ee…`,
7,547 records); the standing DB, manifest, and shard set are untouched
(0 facts → promotion vacuous). Docs + `examples/`-side code only; the
product solver (`src/`), DB schema, and spec are untouched; no in-session
re-propagation.

## Deliverables

- **D1 — union tool**: `examples/proofdb_ledger_union.rs` (4.5 KiB, the
  CLI: `--out <file> <inputs…>`, N ≥ 1, N = 1 normalizes/validates only,
  `--expect <file>=<sha256>` digest pinning verified before the merge) +
  `examples/proofdb/ledger_union.rs` (3.9 KiB, the N-way max-merge with
  the order-independent per-input attribution) + two small enablers in
  `examples/proofdb/ledger.rs` (`Clone`, `from_entries`) + tests in
  `tests/proofdb.rs` (5 new: rule edges incl. pass upgrades / work
  tie-breaks, disjoint paths + N-way order independence, idempotence +
  single-input normalization, the §2.2 coverage property, save roundtrip
  determinism). Public lib API untouched.
- **D2 — the A/B** under `measurements/plan7/`: `census_arm_{1,2}.json`,
  `verdicts.json`, `union.json` (the union artifact record: pinned input
  digests, composition, gates), `ledger_union.json`, per-arm post-run
  ledger snapshots, `env.json`, `README.md` (provenance, command table,
  verdict table), and the `assemble_census.py` driver.
- **D3 — this report.**

## The union (D1 on the real inputs)

Composition — exactly the pinned §1 expectation, checked by digest:

| input | records | new paths contributed | sole pass upgrades |
| --- | ---: | ---: | ---: |
| standing (`058a6202…`) | 4,569 | 391 | 20 |
| plan6 arm A snapshot (`7fd92ea2…`) | 4,933 | **1,381** | **49** |
| plan6 arm B snapshot (`9aa0c326…`) | 4,178 | 0 | 8 |
| **union** | **5,950** | | |

(The standing ledger's own 391/20 are C's ply-5/6 reach, knowledge neither
A nor B has — the union is genuinely N-way, not "standing + A".) Ply-2
split: fresh 190 → **141**, censored 153 → **204** (the 51 A-known
re-censor candidates recovered at pass 1). Gates: input digest
verification, determinism under input reordering, idempotence
(`union(u, inputs)`), N=1 normalization (the standing ledger round-trips
byte-identically), and the §2.2 coverage property — all **pass**,
byte-level where applicable (see `union.json`).

## The A/B (root = startpos; staging copies; cap 300M, base 4M; no mid-run tuning)

| arm | base | jobs | facts | plies visited (jobs) | re-censors (waste) | post-run ledger |
| --- | --- | --- | --- | --- | --- | --- |
| 1 — control | standing (4,569) | 75 (75 expand) | 0 | 2: 24, 3: 24, 4: 24, 5: 3 | **24** (96.0M = 32.0%) | `61097620…` (6,227 records, 300 censored) |
| 2 — union | union (5,950) | 75 (75 expand) | 0 | 2: 24, 3: 24, 4: 24, 5: 3 | **0** (0) | `0d0a86ee…` (7,547 records, 351 censored) |

Session-start censuses matched the pinned §1 lines verbatim (arm 1:
`ledger records 4559 … jobs 4608`; arm 2: `5938 … 5987`; otherwise
identical, cfg echo included). Both arms ran the **identical ply shape**
24/24/24/3 — the layer-visit caps bound identically; the entire
difference is *what the ply-2 ration bought*: arm 1 re-censored 24
A-known-quiet nodes at 4M each, arm 2 censored 24 virgin nodes. H5
replays: job sequences identical (excl `wall_s`), post-run ledgers
byte-identical, both arms.

### The §4.5 decision rule — the union's marginal value

| metric | arm 1 | arm 2 | holds |
| --- | --- | --- | --- |
| re-censors | 24 | **0** | strictly fewer ✓ |
| facts | 0 | 0 | ≥ ✓ |
| max ply reached | 5 | 5 | ≥ ✓ |

**Verdict: the union mechanism ships.** The standing
`data/proofdb_work.json` advanced to arm 2's post-run ledger (sha
`0d0a86ee…`, 7,547 records = 351 censored + 7,196 fresh), and
`proofdb_ledger_union` is the standing-state merge tool — the N-way
primitive item 6 (parallel per-worker ledgers) needs. Arm 2's growth
(+1,597 records: 556 at ply 3, 477 at ply 4, 500 at ply 5, 64 at ply 6)
is all genuinely new knowledge; arm 1's +1,658 contains the same kind of
cascade but bought it after burning 32% of the cap on re-censors.

### The §4.6 closure decision for plan8

Both arms yielded 0 facts — the **sixth consecutive 0-fact 300M
session**. The closure rule fires as pre-registered: **another identical
default batch is not a valid plan8.** Plan8 must change the yield
outlook, choosing between (a) the depth-rationing lever — a fresh-exposure
cap per layer or a ply-front bias (finding 1 of report6: layer visit caps
ration breadth, not depth; both arms here again re-seeded each layer's
fresh pool via the ~21-exposures-per-censor cascade and never completed a
layer or an AND node) — or (b) a fact-yield-oriented selection change.
It needs its own pre-registered plan and design dialogue.

## Gate verdicts

| gate | verdict | notes |
| --- | --- | --- |
| H1 job/shard integrity | **pass** | pinned census lines matched verbatim per arm (cfg echo included); every job path in its arm's post-run ledger; census quadruple consistent (pass-1 visits at number 1 / work_before 0); manifests unchanged; union input digests verified before the merge |
| H2 merge determinism | **pass** | merger ×2 → DB `670e19e3…` and dumps byte-identical both times, = the input DB (0 promoted facts — the censored-session residue check) |
| H3/H4 no-regression / spot-checks | **pass (vacuous)** | 0 facts, 0 shards |
| H5 policy determinism | **pass** | per-arm replays: job sequences identical excl `wall_s`, post-run ledgers byte-identical; **plus union determinism**: reordering → byte-identical, idempotence, N=1 normalization, §2.2 coverage on the real artifacts |
| H6 hygiene | **pass** | `make test` green (50 tests in the proofdb target incl. the 5 new union tests), `cargo clippy --release --all-targets` 0 warnings, `cargo fmt --check` / `cargo doc` clean, no `src/` changes, new example files ≤ 10 KiB, `data/` ignored |

Compute: 4 harvest runs (2 arms + 2 replays) ≈ 4.2 min (arms ran in
parallel) + union runs (seconds) + merger runs + build/test cycles —
within the §8 budget.

## Findings

1. **Finding 4 is now measured, not just estimated: 96,000,242 evals =
   32.0% of the cap.** The plan's ≤ 24 × 4M = 96M bound was tight to
   within 242 evals (the per-job counter overhang). Every ply-2 visit in
   arm 1 was a re-censor; arm 2 redirected the identical ration to virgin
   nodes. The ledger-union advance rule eliminates this waste class
   permanently — and item 6's per-worker ledgers get the merge primitive
   for free.
2. **Ledger growth landed just above the predicted range** (arm 1
   +1,658, arm 2 +1,597 vs the predicted +1,400–1,600), and arm 1's
   growth showed **0 re-exposures against the standing record set** —
   the plan's "~500 ply-3 re-exposures of A-once-known records" are
   re-exposures relative to *A's knowledge*, not to the standing ledger's
   own records (C's advance had dropped them entirely). A definitional
   subtlety worth pinning for future sizing: re-censor waste is measured
   in evals, ledger "re-exposure" in record-set membership, and they do
   not coincide.
3. **The identical 24/24/24/3 ply shapes reinforce report6's finding 1.**
   Recovering the censor knowledge did not change reach at all — it
   changed the *quality* of what the fixed ration bought. A depth-seeking
   policy needs a different lever; the ply-2 fresh head (141 after the
   union) will again re-seed plies 3–6 via the cascade in the next
   default-config session.
4. **The union attribution is symmetric in practice**: the standing
   ledger contributed 391 paths + 20 upgrades that A/B lack (C's deeper
   reach). For item 6 this means per-worker ledger merges must be true
   N-way unions, not "merge workers into the coordinator's ledger".
5. **Base stamping is now a concrete recommendation** (§2.4's deferred
   finding): the ledger format carries no base stamp, so base
   compatibility is enforced only by operator discipline + pinned
   digests (`--expect`). Before item 6 ships parallel harvesters, add a
   base-stamp field (DB digest or manifest digest) to the ledger format —
   a schema-level change that needs its own pre-registered plan.

## Problems encountered

- **A probe-side start-ledger bug** (not a tool defect): the first
  assembly pass read arm 2's *post-run* ledger as its session start (the
  harvest rewrites `--ledger` in place). Caught by the implausible
  +0-growth result; recomputed against the union base. The committed
  driver reads arm 1's start from `../plan6/ledger_snapshot_C.json`
  (byte-identical to the standing ledger at run time) precisely so the
  start state stays pinned after the post-verdict advance.
- **Rust nits caught by the gate**: clippy's `cloned_ref_to_slice_refs`
  (single-input union slice) and a `usage() -> !` coercion rejecting
  `unwrap_or_else` — both fixed; `Ledger` needed `Clone` + a
  `from_entries` constructor for the idempotence test and the union
  output.
- One test expectation error during development (new-path attribution
  counts), caught by the test itself before any run.

## Unresolved / next steps

1. **Plan8 (mandatory per §4.6): the yield-outlook change.** Draft the
   design dialogue first: depth-rationing lever (fresh-exposure cap or
   ply-front bias) vs fact-yield-oriented selection change, sized against
   the measured cascade (~21 exposures/censor) and the 141-node virgin
   ply-2 head.
2. **Item 6 (parallel per-worker ledgers)**: the merge primitive exists
   and is gate-tested; the base-stamp field (finding 5) is its remaining
   prerequisite.
3. Items 3 (DTM-upgrade pass), 5 (website handoff), 7 (subtree scoping)
   unchanged. `make test-full` not required by this session (no `src/`
   or search changes) but stands before any release.

## Tools used

`cargo build/test/clippy/fmt/doc` (release), `python3` (census assembly +
union/ledger probes), the `proofdb_ledger_union`/`proofdb_harvest`/
`proofdb_merge` binaries, `git` (read-only). All runs against throwaway
staging copies in `/tmp/plan7/`; the standing `data/` layer was touched
only by the pre-registered ledger advance; the transcripts live in
`/tmp/plan7/` (regenerable per the README command table; the parsed
census is the record).

SESSION COMPLETE
- `report7.md` written; `measurements/plan7/` complete (2 arm censuses,
  union.json + the union ledger artifact, verdicts.json, 2 post-run
  ledger snapshots, env.json, README, assemble driver); D1
  `proofdb_ledger_union` implemented + tested (5 new tests; gate result:
  H1–H6 all pass, union determinism/idempotence/coverage byte-verified).
  **The §4.5 decision rule fired on all three conditions (re-censors
  24→0, facts 0=0, max ply 5=5): the union mechanism ships** — the
  standing ledger advanced to arm 2's post-run state (`0d0a86ee…`,
  7,547 records). **The §4.6 closure rule fired: plan8 must change the
  yield outlook** (sixth consecutive 0-fact session); the standing DB,
  manifest, and shard set untouched (0 facts, promotion vacuous).
Follow-up options:
1. Kickoff prompt: "Draft the plan8 design dialogue for the proofdb
   initiative (plan session, docs-only): per §4.6 the default batch is
   exhausted — compare the depth-rationing lever (fresh-exposure cap or
   ply-front bias, report6 finding 1) against fact-yield-oriented
   selection changes, sized against the measured ~21-exposures-per-censor
   cascade and the 141-node virgin ply-2 head, and write
   docs/plans/proofdb/plan8.md." — the mandatory next step; the closure
   rule makes another default batch invalid.
2. Alternative: implement the ledger base-stamp field (finding 5) before
   plan8 — worthwhile only if item 6 (parallel harvesters) is being
   pulled forward; it does not address the 0-fact outlook.
