# Report 8 — the `and-close` completion gradient: the first facts of the initiative (arm 2: 34), the deep probes confirm the plateau band

Executes `plan8.md`. All deliverables landed; all pre-registered gates
**pass** except one *plan-model* expectation — **arm 2's ≈ 0-fact forecast
was refuted: the fresh-tail screen produced 34 facts, the first decisive
results of the entire initiative**, at a screen scale where the six
previous 300M `breadth-pns` sessions produced zero. Arm 1's three 1B deep
probes (`g1f3`'s defenses) all censored at their full budget — the first
budgets ever run above the measured censor band on those positions,
establishing measured bounds ≥ 1B child-evals per defense. The §4.1
completion branch did **not** fire (no defense decided, no row
AND-complete); the §4.5 flip analysis reports **0 flips** over the grown
DB; the handover contingency (§4.6) is **not triggered**. The §4.3
standing-mode switch fires on its "only arm 2 yields" branch. Docs +
`examples/`-side code only; the product solver (`src/`), DB schema, and
spec are untouched; decision 10's "flips stay derived" position stands.

## Deliverables

- **D1 — the policy**: `examples/proofdb/and_close.rs` (9.3 KB: the
  `AndCloseOrder` orders, the monotone ladder `ladder_budget`, the
  sequence builder over the classified PNS tree, the census) +
  `and_close/census.rs` (2.7 KB: the census lines + gradient head) +
  `and_close/driver.rs` (6.6 KB: the batch loop, the bump-only censor
  hook, the CLI session assembly) + `Policy::AndClose` + `--and-close-order`
  + the `pass`/`work_before` census fields (with `number`/`kind` null) in
  `session/record.rs` + **7 new tests** in `tests/proofdb.rs` (completion
  order incl. the (missing, ply, path) tie; ladder budgets incl. the
  monotone rule and saturation; fresh order; decisions 9/10 exclusions
  incl. the implied-win cascade; the censor hook's bump-only behavior; the
  driver end-to-end with the no-exposure signature and byte-determinism;
  census lines + parsing). The pns machinery is untouched.
- **D2 — the measurement** under `measurements/plan8/`: per-arm census
  JSONs (784 + 3 job records), post-run ledger snapshots, the union
  ledger (the standing advance), `flip_analysis.json`, `env.json`,
  `verdicts.json`, `README.md` (provenance, command table, verdicts), and
  the `assemble_census.py` driver (all H1/H5/union/H2 checks asserted
  green).
- **D2' — the flip tool**: `examples/proofdb_flip.rs` (12 KB, header
  justified): the §4.5 implied-outcome fixpoint over the grown DB **plus an
  independent recursive verifier** (fresh replay + movegen + completeness +
  bound arithmetic per flip; exit 1 on any unverified claim). It earned its
  keep during development: the verifier rejected the fixpoint pass's first
  polarity-buggy OR-claim on the standing DB — exactly the
  "flip claim without AND-completeness is a defect" tripwire the plan
  demanded — before being fixed to side-to-move semantics with parity
  conversion at output only.
- **D3 — this report.**

## The arms (root = startpos; staging copies; cap 3B each; no mid-run tuning)

| arm | config | jobs | facts | censored | evals | stop |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| 1 — deep probes | `completion`, base 1B, TT 1024 MB | 3 | **0** | 3 | 3,000,000,060 | budget |
| 2 — fresh-tail screen | `fresh`, base 4M, TT 128 MB | 784 | **34** | 750 | 3,001,870,921 | budget |

**Arm 1** ran exactly the pinned sequence — `g1f3 d7d6`, `g1f3 e7e5`,
`g1f3 f7f6`, each censored at 1,000,000,0xx evals (BudgetExhausted,
~230 s wall each ≈ 4.3M evals/s, matching the plan's ~4.6M conversion).
These are the first budgets above every measured censor point on the
quiet class (≥ 10–20M-equivalent standalone, ≥ 4M in-harness): the
plateau band is real, and the three defenses now carry measured bounds
**≥ 1B child-evals ≈ ≥ 4 min wall each** (ledger: pass 1, work ~1B —
the monotone ladder puts their next visits at 2B).

**Arm 2** swept the fresh tail (1,041 fresh replies by §2's definition;
784 visited before the cap) and broke the six-session 0-fact plateau:
**34 decisive replies** — 26 at ply 10, 1 at ply 11, 2 at ply 12, 2 at
ply 14, 2 at ply 17, 1 at ply 18, all refutation steps toward
AND-completion of their parent rows. Per-fact cost: the decisive jobs
spent **1,860,203 evals total (~55k avg, max 893k) = 0.062 % of the arm's
budget** — the facts were effectively free; the 3B went into the 750
censored screens. The facts cluster on 8 parent rows, two of which took
12 and 11 facts respectively (deep slow lines where one side's king walk
is refuted quickly — cheap territory the fresh cascade could never reach
because every one of these nodes sits at proof-number ≥ 2, structurally
unreachable under `breadth-pns`: the starvation mechanism of plan8 §1,
now demonstrated from the other side).

The 12 ply-2 jobs (`g1f3`'s 3 + `g1h3`'s 9 defenses, their first
in-harness probes) all censored at 4M — the head rows are confirmed quiet
at the screen scale; only the 1B tier moves the needle there.

## The standing-mode switch (§4.3, "only arm 2 yields" branch)

The fresh sweep continues at 4M (`order fresh`): arm 2 left 257 fresh
replies unvisited (1,041 − 784) plus the censor-grown tail, and the 34
facts prove that class yields. The completion head escalates in parallel
batches: the three 1B-censored `g1f3` defenses are now pass-1 records
whose monotone-ladder revisit runs at 2B, and `e2e3`/`e2e4`/`g1h3`'s
defenses await their first sub-plateau probes. Per factless-batch
escalation (0 facts everywhere) was pre-registered but did not fire.

## The union advance + promotion (§4.4)

- **Promotion**: 34 validated shards (every one replay-validated by the
  merger) + manifest 197 → 231 entries (`cd84fac7…`); grown DB →
  `data/proofdb.db` (`186d0e89…`, 38,779 nodes = +3,724).
- **Ledger advance**: `data/proofdb_work.json` = union(arm1_post,
  arm2_post) via `proofdb_ledger_union` — the plan7 mechanism's first
  production use — `462467d6…`, 8,214 records = 1,101 censored + 7,113
  fresh. Union inputs digest-pinned; the §2 gates re-run byte-level on
  the real inputs (reversed-order determinism, idempotence, N=1
  normalization). The union correctly kept arm 1's deep-probe knowledge
  (pass 1, ~1B work) over arm 2's 4M bumps at the same pass — the
  monotone-budget state survives the merge, honoring report7 finding 4.

## The flip analysis (§4.5) — the completion question stays open

0 flips over the grown DB (and 0 over the pre-run standing DB): none of
the 75 open rows reached AND-completeness — every touched row still has
censored/unstored replies, and none of the 34 facts is an OR-winning
reply for its parent. The root is undecided; **the handover contingency
(§4.6) is not triggered**; the flip analysis passes vacuously with every
(zero) claim independently verified.

## Gate verdicts

| gate | verdict |
| --- | --- |
| H1 job/shard integrity | **pass** — pinned session-start census per arm (active rows 49, replies 1142, gradient head `g1f3:3 e2e3:7 e2e4:7 g1h3:9 root:13`, exclusions 26/0/0); every budget = the arm base; every job path replayed legally, none a row/manifest path; **no-exposure check exact** (arm 2: post = start ∪ censored paths, 667 new + 83 in-place bumps, max pass 1; arm 1: bumps only) |
| H2 merge determinism | **pass** — merger ×2 (+1 vs promoted manifest) → DB `186d0e89…` + dumps byte-identical; 231/231 shards validated |
| H3/H4 no-regression / spot-checks | **pass** — 34 facts, all shards replay-validated; no same-path outcome contradiction between arms (arm 1 had 0 facts, tripwire vacuous) |
| H5 policy determinism | **pass** — per-arm replays: job sequences identical (excl `wall_s`), post-run ledgers byte-identical — including arm 1's 3B-eval session; **plus union determinism** on the real advance inputs, byte-level |
| H6 hygiene | **pass** — `make test` green (57 tests in the proofdb target incl. the 7 new and-close tests), `cargo clippy --release --all-targets` 0 warnings, `cargo fmt --check` / `cargo doc` clean, no `src/` changes, all new/edited example files ≤ 10 KB except `proofdb_flip.rs` (12 KB, header-justified), `git status` confirms `data/` ignored and the shards/ commit list is exactly manifest + 34 shard bins |
| Flip-analysis consistency | **pass (vacuous)** — 0 flips; the tool's independent verifier is live (it rejected a buggy claim during development, per the gate's defect semantics) |

## Findings

1. **The plan's pinned fresh/censored split (970/172) is a plan-session
   probe artifact — the H1 investigation resolved it as a plan-model
   defect, not a tool defect.** The composition total (1,142), the
   active-row count (49), the gradient head, and all three standing
   digests reproduce exactly; the split does not. From the standing
   ledger (digest-verified `0d0a86ee…`) the reply paths measure 958
   no-record / 83 pass-0 / 101 censored; no natural definition yields
   970/172, and no committed ledger snapshot reproduces it either. The
   implementation follows §2's normative definitions (fresh = no record
   or pass 0 → 1,041; censored = pass ≥ 1 → 101), the census line states
   its own numbers, and the deviation is recorded here rather than
   silently absorbed. The plan-session's throwaway probe was evidently
   run against a different ledger state or lookup rule; its output was
   not committed. Lesson: pre-registered census pins should commit their
   probe output (as this plan's measurement layer now does).
2. **The yield outlook changed sign: cheap facts exist, and the job set
   was the problem.** Six 300M `breadth-pns` sessions: 0 facts. One 3B
   `and-close` fresh sweep: 34 facts for 0.06 % of its evals. The
   difference is exactly the §1 starvation mechanism: the facts sit at
   proof-number ≥ 2 nodes (deep replies of open rows), which the fresh
   cascade never visits because its number-1 pool never empties. The
   plan's design dialogue is confirmed by outcome, not just arithmetic.
3. **The plateau band is confirmed from above**: `g1f3`'s three defenses
   survived 1B child-evals each (~4 min wall, ~200M nodes searched to
   max_depth 41) — one to two orders of magnitude above every prior
   censor point, and still inside the pre-registered plateau class. The
   next escalation rung (2B via the monotone ladder) is pre-registered by
   the standing-mode switch; the 100–250M-node sample bound from the
   solve initiative's 120 s runs suggests these defenses may need the
   full band (7–15B-equivalent) — the per-defense bounds measured here
   are the first hard data on that climb.
4. **The completion gradient's head is stubborn, but the tail pays.** Arm
   2's facts came from rows far down the gradient (ply 9–17 parents), not
   from the near-closure head — consistent with the head's replies being
   genuinely defended and the tail containing cheap refutations. This
   suggests the standing shape should run both scales in parallel
   batches (the §4.3 branch as written) rather than expecting the head to
   crack soon.
5. **`proofdb_flip`'s dual-pass design proved its worth immediately**: the
   independent verifier rejected the first fixpoint's OR-claim (a parity
   bug in root-perspective polarity) on the standing DB — a defect that
   would otherwise have surfaced as a false "root implied" report later.
   The verifier's semantics (AND-completeness mandatory, side-to-move
   polarity, parity at output) are now the tool's contract.

## Problems encountered

- `/usr/bin/time` unavailable in the container (plan7's transcripts used
  shell timestamps instead — cosmetic).
- The census-line assert in the assemble driver initially mirrored the
  plan's 970/172 pin; the H1 investigation (finding 1) replaced it with
  the measured 1,041/101 pin plus an explicit finding record.
- Two nits caught by the gates during development: clippy's
  `filter_map|then` warning in the fresh-order builder, and the flip
  tool's `usage() -> !` coercion (the report7 nit again).

## Unresolved / next steps

1. **Next batch (the §4.3 standing shape)**: (a) the fresh sweep
   continues at 4M over the remaining 257 fresh replies + the censor
   tail — measured-yielding territory; (b) the completion head escalates:
   the three `g1f3` defenses revisit at 2B (monotone ladder), and the
   `e2e3`/`e2e4`/`g1h3`/root replies get their first ≥ 4M probes. Each
   escalation is a new pre-registered batch.
2. **The first materialization candidate**: the 34 facts put 8 rows
   within reach of AND-completeness (two are 12 and 11 facts... i.e.
   refutations, into completion); if the fresh sweep completes any of
   them, the flip analysis produces its first real implied outcome — the
   pre-registered §4.6 handover assessment then has data.
3. Items 3 (DTM-upgrade pass), 5 (website handoff), 6 (parallel
   harvesters; base-stamp prerequisite), 7 (subtree scoping) unchanged.
   `make test-full` not required by this session (no `src/` or search
   changes) but stands before any release.

## Tools used

`cargo build/test/clippy/fmt/doc` (release), `python3` (census assembly +
gate checks), the `proofdb_harvest`/`proofdb_ledger_union`/
`proofdb_merge`/`proofdb_flip` binaries, `git` (read-only). All runs
against staging copies in `/tmp/plan8/`; the standing `data/` layer was
touched only by the pre-registered promotion/advance (§4.4); the
transcripts live in `/tmp/plan8/` (regenerable per the README command
table; the parsed census is the record).

SESSION COMPLETE
- `report8.md` written; `measurements/plan8/` complete (2 arm censuses
  with all 787 job records, 2 post-run ledger snapshots, the union
  ledger, flip_analysis.json, verdicts.json, env.json, README, assemble
  driver). D1 `and-close` implemented + tested (7 new tests; H1–H6 all
  pass, H5 replays byte-identical on both arms including the 3B-eval
  deep-probe session). **34 facts promoted** (the initiative's first);
  standing layer advanced: DB `186d0e89…` (38,779 nodes), manifest
  `cd84fac7…` (231 entries), ledger `462467d6…` (8,214 records). **The
  completion branch did not fire** (0 flips, root undecided, no handover);
  the standing-mode switch fires on its "only arm 2 yields" branch.
Follow-up options:
1. Kickoff prompt: "Draft plan9 for the proofdb initiative (plan session):
   the §4.3 standing shape's first batch — size the parallel fresh sweep
   (4M, remaining ~257 fresh replies + tail) against the completion-head
   escalation (g1f3 defenses at 2B via the monotone ladder; e2e3/e2e4/
   g1h3 defenses at their first sub-plateau probes), pre-register the
   census pins with committed probe output (report8 finding 1), and set
   the factless-batch escalation policy for the plateau climb."
2. Alternative: pull item 6's ledger base-stamp field (report7 finding 5)
   before the next batch if parallel harvesters are imminent; otherwise
   the single-harvester standing shape above has priority.
