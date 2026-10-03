# Report 5a — SPDFPN prototype, stage 2a: `--threads N` mechanism + smoke

Executes `parallel` backlog **#4, stage 2a** per `plan5.md`. One session:
implement the opt-in parallel mode behind `--threads N`, keep the N = 1
surface byte-identical, smoke at 2/4 threads. The GO/NO-GO campaign and the
owner's condition-1 verdict are **plan5b's**; this report hands the mechanism
over with the smoke numbers.

**Verdict: mechanism landed and sound; performance not yet at the GO band.**
Smoke (m22 first-outcome, 5 interleaved reps per N): **outcome agreement
15/15** (threads 2 and 4 always `win`, identical to sequential), zero panics,
zero torn-state symptoms; median speedup **1.42× at 4 threads**, **0.92× at
2 threads** (helpers can hurt), work inflation ~2.2–2.4×, high variance at
N = 2 (one 15 s outlier with 7× coordinator eval inflation). Full drift
protocol green (quick suite 59/59, m22/shuffle-win stdout, snapshot sha — all
identical to the plan4/plan4b records). Gates: `cargo fmt`/`clippy`/`doc`
clean, `make test` green (incl. the new `tests/test_parallel.rs`, 0.4 s).

## What landed

- **`src/search/dfpn/parallel/`** (`mod.rs` coordinator + soundness contract,
  `jobs.rs` virtual TT / job lock / `TRYRUNJOB` dispatch / retirement):
  N−1 helper workers over the shared sharded TT (plan4b), W-capped jobs
  (W = 20 000 child evals), path-derived thresholds, virtual win/loss
  steering (per-worker snapshot, lock-free hot path), post-job
  re-propagation, zero-progress retirement, serialized `ProofEvent`
  forwarding (single forwarder thread → parent sink).
- **dfpn hooks (inert at N = 1)**: `core.rs` frame-entry steering (foreign
  virtual entry → no-store "no progress" return) and the SPDFPN same-child
  resume clause (decision 4, top frame of a job call only, ε-relaxed keep
  rule, AND-node perspective handled via the role-appropriate dn pair);
  `children.rs` child-evaluation steering (synthetic unsolved bounds, never
  an outcome — a virtual entry can never produce a decisive store or event).
- **`Search` changes**: `tt` is now `Arc<TranspositionTable>` (workers share
  it; `Search::new`/`tt()` signatures unchanged), `set_threads` (assert ≥ 1;
  0 panics, tested), `chunk_loop` extracted verbatim from `bounded_search`
  (shared by the N = 1 path and the coordinator).
- **TT additions (parallel-only, never on the sequential path)**:
  `refresh_bounds` (re-propagation: updates pn/dn/best hint of a live
  *unsolved* entry in one shard critical section) and `bump_work`.
- **CLI**: `--threads N` (parse/validation/tests in `cli.rs`; help + header in
  `main.rs` document nondeterminism, the advisory budget, and
  "RAM = TT + O(N) small state").
- **Docs**: `parallel` module header = the soundness contract (what is shared
  vs thread-local, why no false decisive outcome, budget/timeout
  restatements, sequential-owner contract for `clear`/`new_generation`);
  `repetition_cache.rs` gains the per-worker instantiation note;
  `AGENTS.md` architecture + file-size lines updated.
- **Test**: `tests/test_parallel.rs` (fast tier, 0.4 s): N = 1 surface
  equality (outcome + child_evals), 2/4-thread outcome agreement on a win
  and a loss case, repeatability, `set_threads(0)` panic.
- **Measurements**: `measurements/plan5/` (README, env.json, baselines,
  drift results, smoke JSON).

## Smoke results (m22 first-outcome, 30 s cap, interleaved 1/2/4 × 5)

| N | median wall | speedup | outcome agreement | work inflation |
| --- | --- | --- | --- | --- |
| 1 | 3.256 s | — | — | — |
| 2 | 3.535 s | 0.92× | 5/5 `win` | 2.17× |
| 4 | 2.288 s | 1.42× | 5/5 `win` | 2.41× |

Work split (typical t2 run): coordinator 17.7 M evals, helper 15.8 M evals /
6 037 jobs — helpers do real work. Spread at t4: 1.60–3.47 s; at t2:
2.46–15.13 s. The t2 outlier (coordinator_evals 98 M ≈ 7× sequential) shows
helper-stored TT entries steering the coordinator's trajectory into a bad
region — accepted nondeterminism, but the dominant quality risk for plan5b.

**Kill-point check: no decisive disagreement, no panic/torn state — the
prototype is alive; it is below the GO bands, which is plan5b's measurement
to confirm or reject.**

## Deviations from the pre-registered design (declared)

1. **Architecture (the big one).** Pre-registered: workers *replace* the
   sequential chunk loop (paper-faithful worker pool). Landed: the
   **coordinator runs the unmodified sequential chunk loop** while N−1 helper
   workers pre-warm the shared TT. Cause: the pure worker pool could not
   converge on the hard class within this session — instrumented runs showed
   fine-grained jobs re-verifying saturated regions without advancing the
   *global* proof-number state (bounds cycled identically across tens of
   thousands of jobs; e.g. two nodes alternately re-dispatched with unchanged
   `(pn, dn)` forever). The pool's convergence depended on TT-stored
   aggregates standing in for the coordinator's in-call traversal state,
   which they do only coarsely. The helper-pool shape is strictly additive:
   helpers store only path-independent TT payload (the same kind of entries a
   sequential chunk leaves), so the coordinator's convergence is at worst the
   sequential one and outcome agreement is structural, not measured-only.
   Decisions 3–8 are implemented inside the helper pool (W jobs, thresholds,
   virtual TT + job lock + `TRYRUNJOB`, thread-local repetition caches,
   serialized events, wall-clock stop); decision 2 (N = 1 bit-identity) is
   unaffected. Consequence: speedup is bounded by helper usefulness, not by
   pool efficiency — the 1.42×/4 smoke number reflects that.
2. **W = 20 000, not 1 000** (decision 3 deviation). At W = 1 000 a job's
   dive cannot change any stored bounds on the hard class (measured: nodes
   cycled with identical bounds), so jobs degenerated into re-verification.
   20 000 ≈ 4 % of a sequential chunk keeps jobs resumable while productive.
   A coarse 4k/20k/100k probe on m22 showed 20 000 best (single runs, noisy);
   the locked value is plan5b's sweep.
3. **Path-derived thresholds implemented** (not pre-registered either way):
   `TRYRUNJOB` accumulates child-call thresholds exactly like the dfpn
   formulas (OR: `np = min(th_pn, εceil(p2))`, `nd = th_dn − dn + dn_c`;
   AND mirrored). An earlier `(INF, INF)`-threshold variant livelocked on
   lost-looking regions (jobs burned their cap re-diving without bound
   changes); thresholds are what make a W-capped job *resumable*.
4. **No exhaustion claim from helper jobs.** An earlier "root job finished
   under its cap ⇒ exhausted" rule fired prematurely (threshold-cut
   re-verification can produce an all-explored break while deep regions are
   unexplored) — a false proven-Draw hazard, i.e. a soundness violation per
   the plan's kill points. Removed. Exhaustion semantics live entirely in
   the coordinator's sequential chunk loop (sound by construction).
5. **Zero-progress retirement** (new mechanism, not in the paper): a job that
   stored no outcome and left the node's `(pn, dn)` unchanged is retired from
   dispatch for the call (work bump + call-local retired set), because both
   the `work < W` candidacy and the blocker rule would otherwise re-pick it
   forever. Retirement never changes node values (parent dfpn calls still
   expand such nodes per-path with full repetition semantics).
6. **Descent depth cap** (`MAX_DESCENT_PLIES = 12`): the paper's "shallow
   enough for load balance" made explicit — without it the MPN descent
   followed stored best-move chains 100+ plies deep, paying
   O(depth × branching) probes per dispatch.
7. **Repetition handling in the descent** (not in the paper — Hex is
   position-monotone): the descent skips children repeating the seeded path
   and retirement covers the repetition-cache-hit no-store case (the
   first-player-loss GHI shortcut), closing the dispatch-level blind spot the
   journal contract implies.

## Tools / examples used

Standard toolchain only (`cargo`, `make`, `sha256sum`/`cmp`, small Python
drivers for the smoke loop and JSON comparisons; temporary env-gated
instrumentation during debugging, all removed). No new example binaries.

## Problems encountered

- The convergence failures documented under deviations 1/3/4 consumed most
  of the session; each was diagnosed with temporary instrumented builds
  (job logs showing bound cycling, threshold-break rates, and premature
  exhaustion claims) and each fix is evidenced in the session transcript.
- `std::thread::scope` + the coordinator running inside the scope required
  the root template to be cloned before the chunk loop starts mutating the
  coordinator's `Position` (helpers read their own clone; no aliasing).
- Refinement rounds initially hung until the deadline: a round with a smaller
  `max_depth` cannot reuse the root's solved entry (depth guard), so helpers
  churned with no stop signal. Fixed by the coordinator setting
  `shared.stop` when its chunk loop returns (the round is over for helpers).
- The parallel test suite initially took 80 s (same root cause); now 0.4 s.

## Unresolved parts / missing tests

- No torn-state/race stress test for the *helper* path under `make test-full`
  load (the plan allows `#[ignore = "slow: …"]` variants — not written; the
  TT's own concurrency-invariant suite covers the store/probe invariants).
- Helper events under load are exercised only via the small integration
  cases; no proof-tree reconstruction has been run on a `--threads > 1`
  snapshot (out of scope: the search CLI never builds trees, and the offline
  pipeline is plan5b-agnostic).
- The work-inflation metric mixes helper and coordinator evals; a cleaner
  per-phase accounting could be added before the campaign if plan5b wants it.

## Next steps

- **plan5b (pre-registered)**: W sweep (now including, e.g., 8k/20k/50k and
  the coordinator-perturbation angle), interleaved campaign at N ∈ {2,3,4}
  on m22 + shuffle-win, verdict against the GO bands, condition-1
  revert-if-missed execution if NO-GO.
- Candidate mechanism levers if plan5b's sweep is not sufficient (each a
  follow-up plan, not silent scope growth): helper work discounted in TT
  replacement scoring (protect coordinator entries), helper participation
  caps (N−1 → fewer helpers than CPUs − 1 given the forwarder thread), and
  optionally steering helpers away from the coordinator's current chunk.
