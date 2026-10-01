# Plan 8: the yield-outlook change — completion-gradient harvesting (`and-close`) and the first deep-probe session

Initiative: `proofdb`. Executes report7's next-steps 1 under the plan7 §4.6
closure rule (fired: the sixth consecutive 0-fact batch — "another identical
default batch is not a valid plan8"). The plan does three things:

1. **The design dialogue** the closure rule demands: the depth-rationing
   lever (report6 finding 1: fresh-exposure cap / ply-front bias) is
   **rejected on arithmetic** (§2) — it does not touch the structural
   starvation mechanism the six sessions measured. The adopted lever is
   **fact-yield-oriented: change the job set** — a new policy `and-close`
   whose job set is the tree's actual completion work: the missing replies of
   undecided rows (the AND-completion work), ranked by a completion gradient.
2. **The measurement**, two arms: (i) the first **deep-probe session** — the
   near-closure head's defenses at 1B child-evals each, the first budgets
   ever run above the measured censor band on those positions; (ii) the
   **fresh-tail screen** — the never-searched C3 tail (970 replies) at the
   4M scale, the class-closing probe of the last unmeasured cheap-fact class.
3. **The standing-mode switch**, pre-registered for every outcome: `and-close`
   becomes the harvest default (§4.3); a 0-fact outcome switches the standing
   session shape to escalating deep probes (base ×10 per factless batch).

Docs + `examples/`-side code only; the product solver (`src/`), DB schema,
and spec are untouched; decision 10's "flips stay derived" position stands
(§2). Per repo convention the final task is `report8.md`.

## 1. Background (self-contained)

**State after plan7 (verified 2026-10-01, digests recomputed).** Standing
working layer: `data/proofdb.db` sha256 `670e19e3…` (35,055 nodes, 197 shards,
manifest digest `37c1f5b2…` — unchanged since plan3) and
`data/proofdb_work.json` sha256 `0d0a86ee…` — arm 2's post-run ledger,
7,547 records = 351 censored (passes 1–3) + 7,196 fresh:

| ply | fresh | censored | | ply | fresh | censored |
|----:|------:|---------:|---|----:|------:|---------:|
| 0 | 0 | 1 (pass 3, 28M) | | 4 | 922 | 48 |
| 1 | 0 | 20 (13 pass 1, 7 pass 2) | | 5 | 953 | 6 |
| 2 | 117 | 228 | | 6 | 128 | 0 |
| 3 | 5,076 | 48 | | | | |

**The structural starvation mechanism (new; the six sessions' 0 facts are
not bad luck).** Under `breadth-pns`, the queue is `(effective number, ply,
path)` and every censored visit exposes ~21 fresh children at number 1
while raising only its own node to number ≥ 2. The number-1 pool never
empties (plan5 Lemma 2, now measured across six sessions: ~1,600 fresh
records added per 300M session), so **every number ≥ 2 node is unreachable
in-session** — and the number ≥ 2 pool is exactly where the tree's
*completion work* sits: the open rows (the near-closure AND nodes), the
censored nodes' revisits, the root. Eight harvest runs to date (incl.
replays): 439 visits = 430 first-visits of fresh number-1 nodes + 9 rungs;
no node has ever been revisited by the queue. The proof work — visiting the
missing replies that would close AND rows — has been structurally starved
since plan4. This plan changes the job set so the completion work is the
front of the queue.

**The tree's completion work (pinned by probes; the DB is unchanged since
plan3, so these numbers are current).** 75 open rows = 49 active (no proven
ancestor, not implied-decided) + 26 proven-ancestor-excluded. The active
rows' **unvisited replies** — the `and-close` job set — total **1,142** =
970 never-visited + 172 ledger-censored (passes 1–2). The completion
gradient head (rows ranked by missing-reply count):

| row (path) | ply | stored replies | missing | missing replies' state |
|---|---:|---:|---:|---|
| `g1f3` | 1 | 17 (14 × bound 3, 3 × bound 5) | 3 — `d7d6`, `e7e5`, `f7f6` | ledger pass 0 (exposed, never visited) |
| `e2e3` | 1 | 13 (10 × 5, 1 × 7, 1 × 9, 1 × 11) | 7 | all ledger pass 1 (censored at 4M, batches 2–3) |
| `e2e4` | 1 | 13 (10 × 5, 2 × 7, 1 × 9) | 7 | all ledger pass 1 |
| `g1h3` | 1 | 11 (9 × 5, 1 × 3, 1 × 11) | 9 | all ledger pass 0 |
| root | 0 | 7 open children | 13 | ledger pass 2 ×5 (`a2a4`,`b1a3`,`b1c3`,`b2b3`,`b2b4`), pass 1 ×8 |

The head rows are the initiative's most-proving territory: each is a
Black-to-move row whose stored replies are all proven root-player-wins —
completing it stores a "1.X is refuted" line; and completing *any* of the
four head rows implies the **root** (an OR-node with a proven
root-player-win child): the flip analysis (§4.6) is where a startpos result
would first appear — a derived result, reported not materialized (§2).
Conversely, one reply proven root-player-loss flips its row to `'win'` and
implies the root *loss* — both directions are real outcomes of the probes.

**Measured budget bounds on the quiet class (the sizing arithmetic).**
Harness conversion (from the harvest chunk census): 4M child-evals ≈
2–4 × 10⁵ tree nodes, i.e. ~10–23 child-evals/node. The `solve` pilot
(solve plan4) screened **every** ply-1 and ply-2 position standalone at
8 s (~190k nodes/s → 1.4–1.7M nodes ≈ **10–20M child-evals**): 55/400 ply-2
positions decided (all ≤ 1M nodes, all `win`); the rest censored. A seeded
5-position promotion sample re-ran at 120 s (20.5–46.6M nodes ≈
**100–250M child-evals**): 5/5 still censored. The 2-h quiet-root runs
(solve plan3): ~1.4B nodes ≈ **7–15B child-evals**, censored. In this
harness: 4M screens censored on all 439 visits; the 40M heavy tier censored
on every node it ever reached (plan2 deepest-5, plan3 tail sample, plan6
rungs ≤ 16M). So the near-closure head's replies are measured ≥ 4M
in-harness (14 of 26) and ≥ 10–20M-equivalent standalone (all 26);
**arm 1's 1B probes are the first budgets one-to-two orders of magnitude
above every measured censor point on these positions.**

**Why the depth-rationing lever is rejected without an arm.** (i) A
fresh-exposure cap cannot shrink the number-1 pool: unvisited children are
counted structurally via movegen at their parent (plan4 §2), not via ledger
records — capping exposure only slows ledger growth. (ii) Completing a ply
layer is unaffordable at any session scale: the ply-3 pool alone is 5,076
nodes × 4M ≈ 20B evals, and layers grow ~27× per ply (report4 finding 3) —
no fixed cap outruns that. (iii) The ladder/revisit machinery the completed
layers would feed is a measured no-go at its affordable budgets (plan6 §4.5:
9 reserve-bound rungs, all censored, no dividend). Depth-rationing changes
*which* number-1 nodes are visited; it cannot change that only number-1
nodes are visited. The yield lever must change the job set.

## 2. The `and-close` policy (normative, pre-registered)

**Job set.** For every undecided open row that is active (the plan4
decisions 9/10 exclusions verbatim: no proven ancestor, not
implied-decided), its **unvisited legal replies** (movegen at the replayed
position; child paths — neither rows nor manifest paths). This is the C3
extraction with a different ranking and ledger-integrated budgets.

**Ordering (`--and-close-order`, two pre-registered values):**

- `completion` (the standing order): rows ranked by `(missing_count asc,
  ply asc, path asc)`; a row's replies in path-lexicographic order. The
  near-closure head first — the completion work is the front of the queue.
- `fresh` (the arm-2 screen order): only never-visited replies (no ledger
  record, or record at `passes_failed = 0`), ordered by `(reply ply asc,
  path asc)` — the unmeasured tail, shallow-first.

**Budgets.** Per-reply geometric ladder read from the ledger: visit `k =
passes_failed + 1` runs at `budget = max(2^(k-1) × base, work_done)` — the
**monotone-budget rule**, new for this policy: a deep-censored reply
(arm 1, 1B) must not regress to an 8M revisit; the plan4 ladder
(`2^(k-1) × base`) regresses past deep probes otherwise. Base = the CLI
`--budget-evals`.

**Censor behavior (the two normative deltas, superseding plan4 decision 11
*for this policy only*):** a censored reply bumps its own ledger record
(`passes_failed += 1`, work added; atomically saved, crash-safe) and does
**not** expose its children. Rationale, pre-registered: exposure records'
only consumers (the pns queue, pick-up state) derive fresh nodes by movegen
anyway — a pass-0 record is number-equivalent to an unvisited child — so
exposure is bookkeeping with no consumer under `and-close`. The deviation
is measurable: an `and-close` session's ledger growth = its censor count
(~75), not the pns cascade (~1,600).

**Decisive replies** run the standard shard pipeline (class `OpenChild`;
reconstruct → replay-validate → shard + manifest entry, startpos-relative
grafting unchanged). Census records carry `pass` and `work_before`
(ledger-integrated); `number`/`kind` stay null (pns-specific fields).
Session-start census line: `and-close: active rows 49, replies 1142 (fresh
970, ledger-censored 172); order <o>; base budget <b>` plus the gradient
head listing — pinned by §1.

**Decision 10 reconciliation (flips stay derived).** A completed row
(all legal replies decided) is *not* materialized by this plan — the
upward-propagation pass stays future work (report4 next-step 4). The flip
analysis (§4.6) is a read-only census artifact: it reports implied outcomes,
bounds, and the root status; the DB ships the raw tree only, and the spec
(v1) is untouched. Materialization, if ever wanted, is its own pre-registered
plan.

## 3. Deliverables

- **D1 — the policy**: `examples/proofdb/and_close.rs` (≤ 10 KB: the
  sequence builder for both orders, ledger-integrated budgets, census line)
  + `Policy::AndClose` in `policy.rs` + `--and-close-order` in
  `harvest_args.rs`/CLI + the batch driver's censor hook (ledger bump +
  atomic save; no exposure) + tests in `tests/proofdb.rs` (gradient order
  incl. ties, fresh order, ladder budgets incl. the monotone rule, the
  exclusions, the censor hook's bump-only behavior, census echo, CLI
  parsing). The pns machinery is untouched (non-goal).
- **D2 — the measurement** under `measurements/plan8/`: per-arm census JSON
  + post-run ledger snapshots + `flip_analysis.json` (the implied-outcome
  analysis over the grown DB, recomputed independently) + `env.json` +
  `README.md` (provenance, command table, verdicts) + the assemble driver.
- **D3 — `report8.md`**: arm verdicts, the yield outlook's measured state
  (per-fact-cost bounds per defense), the standing-mode switch, gate
  verdicts, findings, and the handover assessment if triggered.

## 4. Pre-registered arms and decisions (fixed before any run)

Both arms run on **staging copies** of the standing state (§1 digests
verified first), root = startpos, `first_outcome_only` on, heavy tier
disabled (`--heavy-sample 0`), fresh TT, and are replayed per H5. Facts are
arm-independent truth and promote after the verdict; a same-path outcome
contradiction between arms aborts the session (the standing tripwire — not
expected: both arms are deterministic from the same inputs).

1. **Arm 1 — the deep probes (the resolution question).**
   `--policy and-close --and-close-order completion
   --budget-evals 1000000000 --max-total-evals 3000000000 --tt-mb 1024`.
   Census expectation: the sequence's first three jobs are `g1f3`'s
   `d7d6`, `e7e5`, `f7f6` at 1B each; the cap fires after them (any
   early-deciding probe shifts budget to the next gradient entries —
   e2e3's replies). Expectations: **≈ 0 facts** (the plateau class:
   these defenses survived 10–20M-equivalent standalone screens); each
   censored probe is a measured bound (≥ 1B child-evals ≈ ≥ 4 min wall on
   that defense). **The completion branch, pre-registered:** if all three
   decide `'win'`, `g1f3` is AND-complete (20/20 replies proven) → the
   flip analysis reports `g1f3` implied `'loss'` (bound = max reply bound
   + 1; stored max is 5 → 6 if the new bounds ≤ 5) and the **root implied
   `'win'` (bound + 1)** — the handover assessment fires (§4.6). A reply
   deciding `'loss'` flips its row to `'win'` and implies the root loss —
   the analysis reports it the same way. 1–2 decisive probes: the
   gradient continues next session (no completion yet).
2. **Arm 2 — the fresh-tail screen (the class-closing probe).**
   `--policy and-close --and-close-order fresh --budget-evals 4000000
   --max-total-evals 3000000000` (TT 128 MB default). Census expectation:
   970 fresh replies; ~75 visits (12 at ply 2 — `g1f3`'s 3 + `g1h3`'s 9,
   their first in-harness probes — then the ply-3 fresh pool: 18+26+18+26+25
   at the first five rows). Expectations: **≈ 0 facts** (the cold-4M
   precedent: plan4's 75 fresh visits → 0); ledger growth **+75** (bumps
   only — the no-exposure signature, checked by H1); ~0 facts closes the
   last unmeasured cheap-fact class at the 4M scale.
3. **The standing-mode switch (pre-registered for every outcome).**
   `and-close` becomes the harvest default in every outcome: the completion
   gradient is the only policy whose job set contains the tree's completion
   work, and the `breadth-pns` fresh cascade is structurally starved (§1).
   The standing *session shape* follows the measurement:
   - arm 1 yields ≥ 1 fact → deep-probe scale stands; the next batch runs
     the gradient head's remaining replies at the measured-viable budget
     (sized in report8 from the per-fact cost data).
   - only arm 2 yields → the fresh sweep continues at 4M (`order fresh`);
     the completion head escalates in parallel batches.
   - **0 facts everywhere → the base escalates ×10 per factless batch**
     (next: 40M), the ladder climbing toward the measured plateau band
     (100–250M sample bound; 7–15B 2-h bound); each escalation is a new
     pre-registered batch. No mid-run tuning (the plan6 rule stands
     verbatim).
4. **Promotion and the ledger advance.** Validated facts from both arms
   promote into the standing shard set + manifest; the standing
   `data/proofdb_work.json` advances to **union(arm1_post, arm2_post)** via
   `proofdb_ledger_union` — the plan7 mechanism's first production use,
   honoring report7 finding 4 ("true N-way unions", not coordinator
   merges). Union inputs pinned by `--expect` digests; the §2 union gates
   (determinism, idempotence, coverage) re-run on the real inputs.
5. **The flip analysis (both arms' grown DB).** A read-only pass over the
   grown DB: for every open row, are all legal replies stored rows with
   proven outcomes? Each complete row's implied outcome + bound (AND-loss:
   all replies root-player-wins, bound = max + 1; OR-win: ∃ proven-loss
   reply, bound = child + 1; the duals), iterated to fixpoint (cascade:
   a flip can imply its ancestors). Output `flip_analysis.json`; any
   implied-decided **root** triggers §4.6.
6. **The handover contingency (pre-registered).** The initiative's handover
   rule (initiative.md): "if the merged tree ever closes the startpos root
   (all ~41 children resolved, OR-node proven), the artifact hands over to
   `solve`". If the flip analysis implies the root: report8 declares the
   derived startpos result with the full implication chain, recomputes it
   independently, and *recommends* the handover; the conservative trigger
   (all children resolved by facts) still requires the remaining root
   children — the harvest continues under the standing policy. No in-plan
   solve reopening; the user decides.

## 5. Pre-registered gates (fixed before any run)

- **H1 integrity:** per arm, the session-start census matches §1's pinned
  numbers (job-set composition 1,142/970/172; the gradient head table; the
  standing digests) — a mismatch is a model defect: stop and investigate.
  Every job path replays legally; no job is a row/manifest path; the
  exclusion census (decisions 9/10) reported. The **no-exposure check**:
  arm 2's post-run ledger growth equals its censor count (bumps only; any
  exposure records = the decision-11 deviation leaking — defect).
- **H2 merge determinism:** merger re-run twice over the grown manifest →
  byte-identical DB + dump; with 0 promoted facts, byte-identity with the
  input DB (the censored-session residue check).
- **H3/H4 no-regression / spot-checks:** verbatim plan4 H3/H4 over promoted
  facts; vacuous at 0.
- **H5 policy determinism:** per-arm full replay (same staging input →
  identical job sequence, byte-identical post-run ledger) — including arm 1
  (the 1B probes are deterministic work; the replay is the session's cost
  doubled, accepted). **Plus the union determinism** on the real advance
  inputs (plan7 §2.3 verbatim).
- **H6 hygiene:** `make test` green (incl. the new and-close tests);
  `cargo clippy --release --all-targets` / `cargo fmt --check` /
  `cargo doc` clean; no `src/` changes; new/edited example files ≤ 10 KB;
  `git status` confirms `data/` ignored.
- **Flip-analysis consistency:** every reported flip recomputed by an
  independent pass and replay-verifiable; a flip claim without
  AND-completeness (the frontier module's assert semantics) is a defect —
  stop.

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 6. Non-goals

No `src/` changes; no schema/spec change (v1 stands; flips stay derived —
the upward-propagation pass stays future work); no pns-machinery changes
(the new policy is additive; `breadth-pns` stays selectable verbatim); no
ladder re-tuning (plan6 §4.5 settled the no-go at its budgets); no child
exposure under `and-close` (§2, decision 11 superseded for this policy
only); no parallel harvesters (item 6 — its base-stamp prerequisite,
report7 finding 5, unchanged); no subtree scoping (item 7); no website
handoff (item 5 — report8's next-steps carries it); no DTM-upgrade pass
(item 3); no in-session re-propagation.

## 7. Tasks

1. Verify the preconditions: the §1 digests recomputed; the census probes
   (frontier counts C1 75 / C2 574,596 / C3 1,789; and-close job set
   1,142 = 970 + 172; the gradient head table) reproduced on staging
   copies.
2. Implement D1 (`and_close.rs` + policy/CLI/census + censor hook + tests);
   run H6.
3. Run arm 2 first (cheap; staging), then arm 1 (the deep probes) — the
   order keeps the expensive arm last so a defect found early costs
   nothing; both replayed per H5.
4. Check gates H1–H6; promote facts; the union ledger advance (§4.4);
   merger runs (H2); the flip analysis (§4.5); assemble D2.
5. Write D3 (`report8.md`): the yield outlook's measured state (per-defense
   budget bounds), the standing-mode switch, the flip/handover assessment
   if any, findings.

## 8. Budget

One session. Implementation: the sequence builder + hook + tests ≈ the
plan6 D1 shape (a few hours). Compute: arm 2 ≈ 70 s (+ replay 70 s);
arm 1 ≤ 3B child-evals ≈ ≤ 15 min at the measured ~4.6M evals/s (+ replay
≤ 15 min); merger + union + census ≈ 5 min. Total ≈ 40–50 min compute —
one sitting, fatter than plan6/7 (the deep probes are the point).
Pre-registered fallback, decided now not mid-run: if arm 1's wall threatens
the sitting, its cap drops to 2B (two probes; the third joins the next
batch) — the completion question then stays open and report8 says so.

## SESSION COMPLETE

- `docs/plans/proofdb/plan8.md` drafted (plan session, docs-only; read-only
  probes on throwaway `/tmp` copies pinned the §1 numbers: standing digests
  DB `670e19e3…` / ledger `0d0a86ee…` re-verified; frontier C1 75 /
  C2 574,596 / C3 1,789; the and-close job set 1,142 = 970 never-visited +
  172 ledger-censored; the completion-gradient head table with stored-reply
  bounds; the near-closure rows `g1f3`(3)/`e2e3`(7)/`e2e4`(7)/`g1h3`(9) +
  the root's 13; the child-evals/node conversion ~10–23 from the chunk
  census; the pilot/120-s/2-h budget bounds on the quiet class re-read from
  the solve initiative's reports).
- Normative content: the design dialogue's verdict (§1/§2 — depth-rationing
  rejected on arithmetic: exposure caps cannot shrink the structural
  number-1 pool, layer completion is unaffordable at 27× growth, the ladder
  is a measured no-go; the starvation mechanism named: the number ≥ 2 pool
  is unreachable under the fresh cascade, which is where all completion
  work sits); the `and-close` policy (job set, two orders, the
  monotone-budget rule, the bump-only censor behavior superseding decision
  11 for this policy); the two-arm measurement (deep probes at 1B on
  `g1f3`'s three defenses — the first budgets above the measured censor
  band there; the fresh-tail screen closing the last unmeasured cheap
  class); the standing-mode switch pre-registered for every outcome; the
  flip analysis + handover contingency; the union ledger advance (the
  plan7 mechanism's first production use). Decision 10 stands: flips stay
  derived, spec untouched.
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan8.md: implement
   `--policy and-close` (+ `--and-close-order`, ledger-integrated ladder
   budgets, bump-only censor hook, tests), run the two-arm measurement
   (fresh-tail screen 4M; deep probes 1B × `g1f3`'s defenses), gates
   H1–H6 incl. the no-exposure check and per-arm replays, promote facts,
   union the ledger, run the flip analysis, report8.md with the
   standing-mode switch and the handover assessment."
2. Alternative: pull item 6's ledger base-stamp field (report7 finding 5)
   ahead of plan8 — worthwhile only if parallel harvesters are being
   prioritized; it does not address the 0-fact outlook, which plan8's
   closure-rule obligation targets.
