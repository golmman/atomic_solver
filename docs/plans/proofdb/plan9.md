# Plan 9: the standing shape's first batch — the fresh-tail sweep and the strictly-growing completion ladder

Initiative: `proofdb`. Executes report8's next-step 1 under plan8 §4.3's
standing-mode switch ("only arm 2 yields" branch, fired by report8: 34
facts from the fresh-tail screen, 0 from the deep probes). The plan is the
**first batch of the standing shape**: two arms on staging copies —

1. **Arm A — the fresh sweep continues** (`order fresh`, 4M): the 257
   never-visited replies left by arm 2, the measured-yielding territory;
2. **Arm B — the completion head escalates**: the gradient head under a
   pre-registered **strict-growth fix to the monotone ladder** — the three
   `g1f3` defenses at 2B each (the first budget above their measured 1B
   censor band), then the sub-plateau tier (`e2e3`/`e2e4`/`g1h3`, 23
   replies at their first 8M rung).

The plan also sets the **factless-batch escalation policy** for the plateau
climb (§4.4) and fixes a latent ladder defect discovered in the plan
session: under plan8's formula, the standing base (4M) revisits a deep
probe at **exactly its accumulated work** — a byte-identical deterministic
repeat, pure waste (§2). Docs + `examples/`-side code only; the product
solver (`src/`), DB schema, and spec are untouched; decision 10 ("flips
stay derived") stands; the handover contingency (plan8 §4.6) carries
verbatim. Per repo convention the final task is `report9.md`.

## 1. Background and pinned state (committed probe output)

**Standing layer after plan8 (verified 2026-10-01, digests recomputed).**
`data/proofdb.db` sha256 `186d0e89a8d92019bd11ed5631280ff924f3c7bcc98818181dabbb185aeb021e`
(38,779 nodes), manifest `docs/plans/proofdb/shards/manifest.json` sha256
`cd84fac7d446ada646f580d0ee7b776909d338a43d4e13cf20786c65c23c7250`
(231 entries), `data/proofdb_work.json` sha256
`462467d64f848f1287c2f6f0ab1844aace260415cc0bdf47a8087b98c5f78296`
(8,214 records = 1,101 censored + 7,113 fresh).

**Committed probe output** (the report8 finding-1 lesson: pre-registered
census pins commit their probe output). Both probes ran on throwaway
`/tmp/plan9probe` staging copies of exactly these digests, 2026-10-01, via
`proofdb_harvest --policy and-close` with a pre-existing stop file (zero
evals spent; verbatim census lines):

```
harvest: db /tmp/plan9probe/proofdb.db (38779 nodes, built_from cd84fac7d446ada6) — policy and-close over frontier C1 75 / C2 641521 / C3 1755 (AND-checks 19398)
and-close: active rows 49, replies 1108 (fresh 257, ledger-censored 851); order completion; base budget 4000000
and-close-gradient: g1f3:3 e2e3:7 e2e4:7 g1h3:9 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4 d2d3 a4b3 e2e3:10 d2d4 a7a6 a2a3 a6a5 b2b3 a5a4 c2c3 a4b3 e2e3:11 root:13 a2a3 a7a6 b2b3 a6a5 c2c3 a5a4:17 d2d4:18 a2a3 a7a6:18
and-close-excluded: open rows 75 (active 49); proven-ancestor 26, implied-win 0, implied-loss 0
```

(`order fresh` prints the identical census line.) Notes:

- The job set is **1,108 = 257 fresh + 851 censored**. The fresh count
  matches report8's 257 exactly; censored grew 101 → 851 by arm 2's 750
  bumps. Frontier C2/C3 moved from plan8 §1's 574,596/1,789 because the DB
  grew by promotion (+3,724 nodes, 34 proven reply subtrees); C1/exclusions
  are unchanged. The current numbers are the pins.
- The gradient head is unchanged from plan8 §1 (`g1f3:3 e2e3:7 e2e4:7
  g1h3:9 … root:13`) plus newly-listed rows at 10/11/17/18 missing.

**Budget table** (from the standing ledger at digest `462467d6…`): every
head-row reply is a pass-1 record except `g1f3`'s three defenses —
`d7d6`, `e7e5`, `f7f6` at **pass 1, work ≈ 1,000M each** (arm 1's 1B
probes); `e2e3` (7), `e2e4` (7), `g1h3` (9), the 10-missing row (10), the
11-missing row (11), and the deep rows (17/18/18) at pass 1, work 4M; the
root's 13 missing replies split pass 1 ×13 / pass 2 ×7 (work 4M / 12M).

**Fresh-order head probe** (one job run on the staging copy, committed):
the first fresh jobs are **ply-19** quiet ladder replies (`d2d4 d7d5 …
c1d2`, `d2d4 d7d5 … d1b3`), censored at 4M — the remaining 257 are the
deepest end of the pool (the arm-2 sweep was ply-ascending and stopped at
the 3B cap). The arm-2 yield data (34 facts; deepest at ply 18) keeps a
positive prior on this tail.

**The ladder defect (plan-session finding).** Plan8 §2's monotone rule is
`budget = max(2^(k−1) × base, work_done)`. With the standing base 4M,
`g1f3`'s defenses (pass 1, work ≈ 1B) revisit at `max(8M, 1B) = 1B` —
**exactly their accumulated work**. A job's search is a fresh, empty-TT
run capped by cumulative child-evals (`Session::run_job_with_budget` →
`set_child_eval_budget`); a rerun at budget = work_done reproduces the
original run byte-for-byte (plan8 H5 proved arm-1's 3B-eval session
replays byte-identically) and re-derives the same censored result: **pure
waste, ~3B evals per completion arm**. The 2B rung report8 pre-registered
("the three 1B-censored defenses … revisit at 2B") is reachable only with
base 1B — but base 1B makes *every* pass-1 tail reply revisit at 2B,
blindly jumping 500× past their measured 4M censor. Neither base serves
both tiers; the formula is wrong, not the bases.

## 2. The strict-growth ladder (normative, pre-registered)

Supersedes the floor clause of plan8 §2 **for `and-close` only** (the
formula's only consumer): a reply's k-th visit (`k = passes_failed + 1`)
runs at

```
budget = max(2^(k-1) × base, 2 × work_done)     (k ≥ 1)
budget = base                                    (k = 0)
```

— the floor **doubles** instead of matching accumulated work. Rationale,
pre-registered: a revisit at budget ≤ work_done is a deterministic repeat
(§1) and must be impossible; `2 × work_done` guarantees strict growth for
every pass ≥ 1 record, is base-independent at the deep end, and reduces to
plan8's behavior wherever the old floor was strictly exceeded (pass-1
4M-tail replies: `max(8M, 8M) = 8M`, unchanged). Consequences pinned:

- `g1f3` defenses at base 4M: `max(8M, 2 × 1,000,000,0xx) ≈ 2B` — the
  report8 pre-registered rung, reached with the standing base.
- The cumulative rung ladder for a deep probe: 1B (plan8, measured) → 2B
  (this plan) → **6B** next (2 × (1B + 2B)) → 12B — geometric in
  cumulative work, climbing toward the measured plateau band (the solve
  initiative's 2-h quiet-root bound, 7–15B-equiv).
- Fresh (pass 0) replies: base, unchanged.

Implementation: `ladder_budget` in `examples/proofdb/and_close.rs` (one
line), its doc comment (cite plan9 §2 alongside plan8 §2), and the ladder
tests in `tests/proofdb.rs` (the monotone-rule expectations become the
strict-growth expectations; add the equal-work regression case: pass 1,
work = 2^(k−1) × base must yield 2 × work, not work). No other policy
code changes; `breadth-pns` and the censor hook are untouched.

## 3. Deliverables

- **D1 — the ladder fix** (§2) + updated tests. No new CLI surface.
- **D2 — the measurement** under `measurements/plan9/`: per-arm census
  JSONs, post-run ledger snapshots, the union ledger (the standing
  advance), `flip_analysis.json`, `env.json`, `verdicts.json`,
  `README.md` (provenance, command table, verdicts), and the assemble
  driver.
- **D3 — `report9.md`**: arm verdicts, the plateau climb's state (the
  defenses' measured bounds after this batch), the escalation policy's
  first application, gate verdicts, findings, and the handover assessment
  if triggered.

## 4. Pre-registered arms and decisions (fixed before any run)

Both arms run on **staging copies** of the standing state (§1 digests
verified first), root = startpos, `first_outcome_only` on, heavy tier
disabled (`--heavy-sample 0`), fresh TT, and are replayed per H5. Facts
are arm-independent truth and promote after the verdict; a same-path
outcome contradiction between arms aborts the session (the standing
tripwire). Arm A runs first (cheap), then arm B (the expensive arm last —
the plan8 ordering rule).

1. **Arm A — the fresh sweep.** `--policy and-close --and-close-order
   fresh --budget-evals 4000000 --max-total-evals 1100000000` (TT 128 MB
   default). Census expectation (H1): 257 jobs, every budget 4M, every
   `pass` before-bump 0. The cap covers the full sweep (257 × 4M ≈
   1,028M; decisive jobs cost less). Expectation: **several facts** (the
   arm-2 prior: 34/784 ≈ 4.3% → ~5–15; the tail is deeper, plies ≥ 19, so
   the low end is likelier); ~257 censored; ledger growth = censor count
   (the no-exposure signature). **0 facts is a valid measured result**
   (the cheap-fact class ends at ply 18) — it escalates the fresh tier
   per §4.4, it is not a defect.
2. **Arm B — the completion head escalation.** `--policy and-close
   --and-close-order completion --budget-evals 4000000
   --max-total-evals 6500000000 --tt-mb 1024`. Census expectation (H1):
   the sequence head is exactly `g1f3 d7d6`, `g1f3 e7e5`, `g1f3 f7f6` at
   budgets ≈ 2B each (2 × work_done per §2), then `e2e3`'s 7, `e2e4`'s 7,
   `g1h3`'s 9 replies at 8M each (26 jobs ≈ 6,184M), then the 10-missing
   row's replies at 8M until the cap fires (~65 jobs total). Any early
   decisive probe frees budget and the gradient continues deeper — the
   cap, not the sequence, is the stop. Expectations: **≈ 0 facts** from
   `g1f3` (the plateau band: ≥ 1B measured, the band estimate says the
   full climb may be needed) and a small chance on the sub-plateau tier
   (pass-1 replies, censored at 4M in batches 2–3 — the arm-2 surprise
   tempers but does not transfer the fresh-rate prior). Each censored
   probe is a measured bound; `g1f3`'s defenses advance to pass 2,
   work ≈ 3B.
3. **The completion branch** (plan8 §4.6, verbatim): if any `g1f3`
   defense decides `'win'` and the row's replies are all proven
   root-player-wins, `g1f3` is AND-complete → the flip analysis reports
   the implied outcome + bound and the **handover assessment fires**; a
   reply deciding `'loss'` flips its row to `'win'` and implies the root
   loss — reported the same way. Flips stay derived (decision 10); the
   conservative handover trigger (all root children resolved by facts)
   is unchanged.
4. **The escalation policy for factless batches** (the standing shape's
   standing rule, reconciling plan8 §4.3's ×10 rule with §2's ladder):
   - **Fresh tier** (pass-0 pool): 0 facts at base *b* → the next batch's
     fresh-tier base is ×10 (plan8 §4.3 verbatim; after arm A's 0-fact
     outcome: 40M).
   - **Censor tail and head** (pass ≥ 1): the strict-growth ladder
     escalates automatically (every revisit ≥ 2 × cumulative work); no
     same-budget revisit is ever scheduled. `g1f3`'s rung ladder is
     pinned: 1B (measured, plan8) → 2B (this plan) → 6B → 12B, toward the
     7–15B-equiv band; each rung is a new pre-registered batch.
   - Both tiers factless → both escalate per the above; one batch per
     escalation, no mid-run tuning (the plan6 rule stands verbatim).
5. **Promotion and the ledger advance.** Validated facts from both arms
   promote into the standing shard set + manifest; the standing
   `data/proofdb_work.json` advances to **union(armA_post, armB_post)**
   via `proofdb_ledger_union` (inputs digest-pinned by `--expect`; the
   plan7 §2 union gates re-run byte-level on the real inputs — the
   mechanism's second production use). The arm job sets are disjoint by
   construction (A: fresh-only, ordered by ply; B: the gradient head);
   any overlap the union's max-rule absorbs (passes max, tie by
   work_done).
6. **The flip analysis** (plan8 §4.5, verbatim): `proofdb_flip` over the
   grown DB after promotion; every reported flip independently verified
   by the tool's recursive verifier; any implied-decided **root** fires
   the handover assessment (§4.3's conservative trigger unchanged).

## 5. Pre-registered gates (fixed before any run)

- **H1 integrity:** per arm, the session-start census matches §1's
  committed pins (job set 1,108/257/851; active rows 49; the gradient
  head string; exclusions 26/0/0; the standing digests) — a mismatch is a
  model defect: stop and investigate. Arm A: every job budget 4M,
  before-bump pass 0. Arm B: the sequence head is exactly the §4.2 pin
  (three ≈2B probes then the 8M tier; the budget echo carries `pass` and
  `work_before`). Every job path replays legally; no job is a row/manifest
  path. The **no-exposure check**: each arm's post-run ledger growth
  equals its censor count (bumps only).
- **H2 merge determinism:** merger re-run twice over the grown manifest →
  byte-identical DB + dump; with 0 promoted facts, byte-identity with the
  input DB.
- **H3/H4 no-regression / spot-checks:** verbatim plan4 H3/H4 over
  promoted facts; vacuous at 0.
- **H5 policy determinism:** per-arm full replay (same staging input →
  identical job sequence, byte-identical post-run ledger) — including arm
  B's 6.5B-eval session (the replay doubles the arm's cost, the plan8
  precedent, accepted). **Plus the union determinism** on the real
  advance inputs (plan7 §2.3 verbatim: reversed-order determinism,
  idempotence, N-way normalization).
- **H6 hygiene:** `make test` green (incl. the updated ladder tests);
  `cargo clippy --release --all-targets` / `cargo fmt --check` /
  `cargo doc` clean; no `src/` changes; new/edited example files ≤ 10 KB;
  `git status` confirms `data/` ignored.
- **Flip-analysis consistency:** plan8's gate verbatim (AND-completeness
  mandatory; a flip claim without it is a defect — stop).

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 6. Non-goals

No `src/` changes; no schema/spec change (v1 stands; flips stay derived);
no pns-machinery changes; no parallel harvesters (item 6); no subtree
scoping (item 7); no website handoff (item 5); no DTM-upgrade pass
(item 3); no ladder *schedule* re-tuning beyond the §2 floor fix (plan6
§4.5's no-go stands at its budgets); no materialization of flips; no
in-session re-propagation; no revisiting of the completion-gradient head
beyond this batch's rungs (each escalation is its own pre-registered
batch).

## 7. Tasks

1. Verify the preconditions: the §1 digests recomputed; the census probe
   (job set 1,108 = 257 + 851; the gradient head; exclusions) and the
   budget table reproduced on staging copies.
2. Implement D1 (the §2 floor change + tests); run H6.
3. Run arm A, then arm B (staging); both replayed per H5.
4. Check gates H1–H6; promote facts; the union ledger advance (§4.5);
   merger runs (H2); the flip analysis (§4.6); assemble D2.
5. Write D3 (`report9.md`): the arm verdicts, the plateau climb's state,
   the escalation policy's application, findings.

## 8. Budget

One session. Implementation: the §2 one-line change + test updates
(≈ 1 h). Compute: arm A ≈ 4 min (+ replay ≈ 4 min); arm B ≤ 6.5B
child-evals ≈ ≤ 25 min at the measured ~4.3M evals/s (+ replay ≤ 25 min);
union + merger + census ≈ 5 min. Total ≈ 65–70 min compute — one sitting,
fatter than plan8 (the 2B probes are the point). Pre-registered fallback,
decided now not mid-run: if arm B's wall threatens the sitting, its cap
drops to 4B (two `g1f3` probes; the third and the 8M tier join the next
batch) — the completion question then stays open and report9 says so.

## History

- **2026-10-01 — drafted (docs-only plan session).** Probes on throwaway
  staging copies pinned the standing census (committed in §1) and
  discovered the equal-budget-revisit ladder defect (§1/§2). See §2–§8.

## SESSION COMPLETE

- `docs/plans/proofdb/plan9.md` drafted (plan session, docs-only;
  read-only probes on throwaway `/tmp` copies, **committed in §1** per
  report8 finding 1: standing digests re-verified (DB `186d0e89…`,
  manifest `cd84fac7…`, ledger `462467d6…`); the and-close census 1,108 =
  257 fresh + 851 censored, gradient head unchanged plus 10/11/17/18
  rows; the ledger budget table (g1f3 pass 1 ≈ 1B ×3; the sub-plateau
  tier pass 1 4M; root pass 1/2 mixed); the fresh-order head at ply 19).
- Normative content: the **strict-growth ladder** (§2 — the plan8 floor
  `work_done` → `2 × work_done`; rationale: a revisit at budget ≤ work is
  a byte-identical deterministic repeat, plan8 H5's replay is the
  evidence; pinned consequences: `g1f3` at 2B with the standing base, the
  cumulative rung ladder 1B → 2B → 6B → 12B); the two-arm batch (§4: the
  257-job fresh sweep at 4M; the completion head at 2B ×3 + the 8M
  sub-plateau tier, cap 6.5B); the **factless-batch escalation policy**
  reconciled (§4.4: fresh base ×10, censor tier automatic via the
  ladder); promotion + the union ledger advance (second production use);
  the flip analysis + handover contingency verbatim. Decision 10 stands.
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan9.md: implement the
   strict-growth ladder floor (2 × work_done) with updated tests, run the
   two-arm batch (fresh sweep 257 × 4M; completion head g1f3 ×2B + the
   8M sub-plateau tier, cap 6.5B), gates H1–H6 incl. per-arm replays and
   the no-exposure check, promote facts, union the ledger, run the flip
   analysis, report9.md with the escalation policy's application."
2. Alternative: pull item 6's ledger base-stamp field (report7 finding 5)
   before the next batch — worthwhile only if parallel harvesters are
   imminent; it does not advance the plateau climb, which plan9 targets.
