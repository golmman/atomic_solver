# Plan 2: Design-space note + option-C sizing spike (architecture selection)

Initiative: `parallel` backlog #2. This plan produces three decisions the
initiative owes (opening paragraph of `initiative.md`, Status section):

1. **Does an opt-in nondeterministic parallel mode ship at all?** (lean
   trigger 1, consciously deferred here by the initiative's opening.)
2. **Which shape** — a harness example over unmodified solver processes
   (option C), a CLI `--threads N` mode (option A), both, or neither.
3. **GO/NO-GO for backlog #3** (staged implementation), with
   pre-registered decision rules so the outcome is measured, not argued.

Deliverables: `design_space.md` (the architecture note, this directory),
the option-C sizing spike (measurements only, artifacts under
`measurements/plan2/`), backlog/initiative updates, and `report2.md`.

**No `src/` changes. No `examples/` changes. No drift exposure**: the
spike drives the existing release binary as a black box; the sequential
path cannot drift because nothing in the build changes. A post-spike
drift sanity run (task 10) is still performed as hygiene.

Plan 1's synthesis is input, not pre-decision: C first (harness over
unmodified processes), A behind it (gated on mining Pawlewicz & Hayward
2014), B as adjunct, D dead. This plan confirms or overturns that with
our own measurements on our own tree shape.

Prerequisite reading:

- `parallel/initiative.md` — design constraints 1–4 are normative; the
  option-space section (with plan 1's cross-paper comparison) is the
  starting point of the note.
- `parallel/research_cizek2025.md`, `parallel/research_jlpns.md`,
  `parallel/research_solrex.md` — per-paper mechanisms and small-pool
  extrapolations; the published bands this plan's measurements are
  compared against (work inflation ≈1.0–2.5×; near-linear job-level
  scaling at 4–16 workers).
- `lean/report7.md` — the deterministic no-go (do not re-litigate), the
  ~40% OR / ~60% AND work split, and the OR-early-exit tail risk that
  spike item (i) must quantify.
- `dfpn/research_ghi_journal.md` — why constraint 2 makes every
  shared-TT / shared-mutable-state variant (A, D) expensive.
- `research/structural_floor.md` §2, §9 — the cross-worker Win/Loss
  reuse ban and why parallelism is the last multiplicative lever.
- `examples/find_winning_child.rs`, `examples/list_legal.rs`,
  `examples/replay.rs` — the spike's building blocks (below).

## Hardware envelope (honest constraint)

The reference container has **4 cores / 16 GB RAM**. Wall-speedup
measurements are meaningful only at N ≤ 4 workers; N = 8/16 projections
come from a deterministic schedule simulation over the measured
per-child timing/node distribution (task 8), clearly labeled as
simulated in `design_space.md` and the report. If a future host offers
more cores, the simulation is replaced by measurement — nothing in the
decision logic changes.

## What option C means *concretely* on this solver

Spelled out in the note (Phase 0) and implemented by the spike:

- The root of a solve is an **OR node**: the first root child proven
  winning proves the root. So the simplest C instance is a **root-child
  race over process-per-child workers**: enumerate root moves
  (`list_legal`), derive each child FEN (`replay <fen> <move>` — it
  prints `fen:`), run each child as its own solver process, first
  decisive child wins; workers are the *unmodified* sequential solver.
- Constraints 1/2 are **vacuous at the worker level** (each worker is
  today's solver, bit-identical semantics, private TT, no sharing of
  anything mutable). The master combines only decisive facts, which is
  rule-sound composition, not cross-worker *reuse* (the plan10 ban is
  about caching results into a TT/proof state, not about a scheduler
  observing worker outcomes).
- **Outcome-perspective mapping** (the classic bug source; the harness
  must encode it explicitly and be cross-validated): a root move is
  proven winning iff the child position is terminal (already decided,
  `replay`'s `pos.outcome()`) **or** the child solve (child's side to
  move) returns `loss`. Child `win` ⇒ the root move loses for the root
  side (eliminated, not proven). Child `draw` ⇒ unproven (consumed as
  such, per the initiative's measurement conventions). A root is
  **disproven** only if all children return `win`/eliminated — not a
  spike goal, but the harness should report it when it happens.
- Master-side re-direction (PPN₂-style partial pn/dn feedback) is **out
  of spike scope**; it is recorded in the note as the enrichment path
  between this spike's plain race and JLPNS proper.

## Phase 0 — desk work: the comparison matrix and the ship shape

Write the `design_space.md` skeleton:

1. **Options A–D × constraints 1–4 matrix**, incorporating plan 1's
   synthesis verbatim where it settles a cell; every cell cites either
   a research file, `lean/report7.md`, or a spike measurement.
2. **The ship-shape decision space**, stated before measurements:
   - `examples/parallel_solve`-style harness (process race): CLI
     untouched — constraint 1 holds by construction ("with the option
     off, every stdout byte matches today" is trivially true because
     there is no option); RAM restated in the *example's* docs as
     N×TT, not in the search CLI's contract; no proof-event surface.
   - CLI `--threads N`: touches `src/main.rs` + `src/cli.rs` help, the
     drift gate, the RAM=TT contract, and the budget-determinism
     surface (constraint 3) — only justifiable if A advances.
   - Both / neither, with the consumer story for each (who runs this?
     the external optimizer's `benchmark` harness is explicitly
     sequential-deterministic; the realistic consumer is a human
     analyst on a multicore box — say so).
3. **Bibliography entry** for Pawlewicz & Hayward 2014 (*Scalable
   Parallel DFPN Search*, CG 2013, LNCS 8427) added to
   `docs/bibliography.md` as **Open** — mining is deferred to the
   A-stage plan that would need it; do not mine it here.
4. The D-is-dead and B-as-adjunct verdicts re-checked against the
   matrix (they rest on plan 1's reading, which is fine — but the note
   must own them, citing `research_cizek2025.md` §option-D and
   `research_jlpns.md`).

## Phase 1 — sizing spike (order per the initiative's backlog #2)

All artifacts under `measurements/plan2/` (script + raw captures +
README with the command table, following the layout of
`lean/measurements/plan1/`). The harness is a **Python 3 stdlib-only
script** that spawns the release binary; it is a measurement artifact,
not shipped code. If backlog #3 later GOes, plan 3 promotes the logic
into a real example binary — that decision is why the spike script must
be clean, readable, and perspective-correct.

### (0) Baselines

- Sequential root solves: m22_white
  (`4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`,
  `--timeout 30 --first-outcome --outcome-only`) and shuffle-win
  (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`,
  `--timeout 100 --first-outcome --outcome-only`). Record wall,
  outcome, PV length.
- Per-child sequential solves for every root child of both cases:
  `replay` for the child FEN (captures terminal children immediately),
  then the solver with a per-child cap of 120 s. Record wall, outcome,
  and work. Work accounting: run the per-child solves twice —
  `--outcome-only` for wall, and a non-outcome-only pass parsing
  `pre_exit: ... nodes=N` (`search.nodes()`, dfpn entries) for the
  work proxy. `child_evals` per se is not CLI-visible; `nodes` is the
  documented proxy (state this in the README and the note).
- Perspective-mapping validation: on a small shallow FEN (pick a quick
  suite case where children finish in milliseconds), verify the
  harness's proven-root-move set equals `find_winning_child`'s exactly
  before any timed run.

### (i) Tail/utilization and work inflation for C at 4–16 workers

- Race harness: N workers (2, 3, 4), per-child cap as above, first
  proven child stops everything. Measure: wall speedup vs the
  sequential root solve; **tail** = time from the first decisive child
  finishing to the race end under an N-worker schedule (computed from
  the per-child timing distribution — this isolates the OR-early-exit
  waste lean report7 flagged); **aggregate work inflation** = total
  worker `nodes` (all started children, finished or not) ÷ sequential
  root `nodes`.
- ≥5 repeated race runs per N on m22 for nondeterminism statistics.
- Shuffle-win probe: same harness, but expect most children not to
  finish within sane caps; record raw data and mark inconclusive
  results as such rather than extending budgets unboundedly (m22 is
  the primary quantitative case; say why in the README: its sequential
  root solve is ~an order of magnitude cheaper, so per-child budgets
  close).

### (ii) Decisive-composition agreement

- Every child's race outcome (decisive or not) vs the baseline
  per-child solve, and the union of proven root moves vs
  `find_winning_child` at matched budgets. The invariant to check is
  **never a decisive disagreement** (no child proven both ways, no
  false proven root); decisive-vs-undecided differences from budget
  mismatch are recorded, not treated as failures.
- 5× repeated-runs agreement on m22 at N=4: same proven root move set
  every run (or document the variation).

### (iii) RAM at N×TT

- Peak RSS per worker (`/usr/bin/time -v` or `ru_maxrss` from the
  harness) at the default 128 MB TT; practical maximum N before
  violating a 16 GB-class envelope; the constraint-4 restatement text
  drafted for the note ("parallel mode RAM ≈ N × TT + per-process
  overhead").

### (iv) 8/16-worker projection (simulation, labeled as such)

- Deterministic schedule simulation over the measured per-child
  (wall, nodes) distribution: projected speedup and tail sensitivity
  at N = 8, 16. No oversubscribed wall runs — they would measure the
  container, not the architecture.

## Phase 2 — decision (pre-registered rules)

Evaluated on m22 at N=4 (median across ≥5 runs):

- **GO** (backlog #3 unblocked, C-harness shape): wall speedup
  **≥ 2.0×** AND aggregate work inflation **≤ 2.5×** AND zero decisive
  disagreements / zero false proven roots in (ii). Rationale for the
  bar: below 2× at 4 workers the lever cannot reach the initiative's
  2–8× goal even at 16 (given published saturation), so L-effort
  implementation is not justified; 2.5× inflation is the worst band
  Čížek observed (0.94→2.50 across 1→1024 cores) — beyond that we are
  measurably worse than the literature's design on our own tree.
- **MARGINAL** (speedup in [1.5×, 2.0×) with inflation ≤ 2.5×): the
  numbers are recorded and backlog #3 stays blocked pending the
  consumer story (lean trigger 1); the initiative stays active with
  the note as its state of record.
- **NO-GO** (speedup < 1.5×, or inflation > 2.5×, or any soundness
  violation): backlog #2 closes as measured no-go; the initiative
  moves toward closure with the record (README row updated, since
  that is a pivot/close event).

Independently of GO/MARGINAL/NO-GO, the note records the **ship-shape
decision** and the constraint-4 RAM restatement. Working hypothesis
(overturnable by the data): ship as an example harness, never as a CLI
flag; A remains a separate, gated future stage.

## Tasks

1. Phase 0: add the Pawlewicz & Hayward 2014 bibliography entry
   (**Open**); write the `design_space.md` skeleton (matrix + ship
   shapes + root-race definition + perspective mapping).
2. Create `measurements/plan2/` with the baseline captures (sequential
   roots + per-child tables + `pre_exit` work proxies) and a README
   command table.
3. Write the race harness script (stdlib-only Python; perspective
   mapping explicit; per-child caps; RSS capture).
4. Validate the harness on a shallow quick-suite FEN against
   `find_winning_child` (exact agreement on the proven-move set).
5. Run the races: m22 N ∈ {2,3,4} × ≥5 reps; shuffle-win probe; capture
   raw stdout/stderr per run.
6. Run the composition-agreement checks (task (ii) above) and the RAM
   measurement (task (iii)).
7. Run the N=8/16 schedule simulation; write the projection with its
   label and sensitivity notes.
8. Complete `design_space.md`: fill the measured cells, apply the
   Phase 2 rules, write the decision and the ship shape.
9. Update `parallel/initiative.md`: backlog #2 status (GO/MARGINAL/
   NO-GO + pointer to the note and measurements); enrich or close
   backlog #3 accordingly; append the decision to History. Update
   `docs/plans/README.md` **only** if this is a pivot/close (NO-GO), not
   for a plain GO.
10. Hygiene: `git status` shows only docs/measurements changes; run the
    m22 first-outcome drift capture against the stored baseline
    (byte-identical; nothing should have changed — this run exists to
    prove the spike left no residue).
11. Write `report2.md`: measured numbers (speedups, inflation, tail,
    agreement stats, RAM), the decision, discrepancies vs the published
    bands, problems encountered, missing tests (the harness is
    unvalidated beyond task 4 — note what a real implementation must
    test), and next steps (plan 3 scope if GO; closure steps if NO-GO).

## Non-goals

- Any `src/` or `examples/` change; no `--threads` flag; no TT,
  proof-event, or proof-tree changes.
- PPN₂-style master re-direction or any shared-state protocol — noted
  as the enrichment path, not built.
- Mining Pawlewicz & Hayward 2014 (bibliography entry only) — an
  A-stage plan's first task, per report1's next steps.
- Re-litigating the deterministic-parallelism no-go (`lean` plan7) or
  the cross-worker Win/Loss-reuse ban (plan10 hazard class).
- Default-on parallelism in any form.
- Publishing 8/16-worker wall numbers as measurements (simulation only,
  labeled).

Per repo convention, the final task of this plan is writing its
`report2.md` in this directory.
