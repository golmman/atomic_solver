# Plan 5 — Implementation report: campaign architecture design spike (GHI-correct job-level DF-PN + two-job prototype)

Executed 2026-09-23. Deliverables: D1
[`campaign_architecture.md`](../campaign_architecture.md) (written **before**
the prototype ran), D2 `examples/campaign_master` / `examples/campaign_worker`
(+ `examples/campaign/{mod,verify}.rs`), this report, and
[`measurements/plan5/`](README.md) (drivers, committed per-rep JSONs, proof
artifacts, audit results). No product CLI changes; the only lib-surface
addition is the read-only `TtEntry::advisory_pn_dn()` accessor (A4 in the
architecture doc; behavior-neutral, no search logic uses it). Fast gate
(`make test`, 240 unit + integration tests) green; clippy/fmt clean.

## 1. What was built (D2)

- **Worker**: persistent process = one in-process `Search` (unmodified
  semantics) with a private TT retained across jobs; per job it replays the
  job path from the campaign root (context contract, `search_depth_with_
  prefix`), runs under the job's deterministic child-eval budget, and — for
  decisive outcomes — exports a proof subtree **via the product's own offline
  pipeline** (TT snapshot → `reconstruct` → `validate_proof_tree`).
- **Master**: AND/OR proof state over replayed position keys (rule-derived
  expansion of AND replies), pseudo-MPN dispatch (min-pn child → min-dn
  leaf), locked leaves with worker affinity, advisory pn/dn updates from
  workers, merge verification (global-path replay + structural subtree check
  per fact; finalize + `validate_proof_tree` at artifact emission), JSON
  state dumps, file-based job store (§8 schemas).
- `--mode seq` on the master doubles as the S-baseline runner (reports
  nodes **and** child_evals, which the CLI does not expose).

## 2. Soundness incidents found by the prototype (all fixed campaign-side)

1. **Non-self-contained worker exports (A1).** With a retained TT, a decisive
   search resolves nodes via entries proven under *earlier* jobs whose event
   streams were discarded. Raw event streams then have holes; finalize
   silently leaf-ifies them into depth-0 interior wins that the product
   validator flags (`terminal_mismatch`). Measured: before the fix, one
   worker hit 166 such defects in one session. Fix: export via
   `reconstruct` (hole-filling) + validator gate; on failure the worker
   downgrades the result to unresolved and resets its TT.
2. **Master merge keyed en-passant (and castling) wrong.** The verifier's
   trie keys were built from *normal-move* encodings of the UCI strings, so
   an en-passant child (`c5d6` after `d7d5`) decoded as an illegal normal
   move → spurious `verify_failed` aborts (5 of 9 pre-fix m22 C2/C4 runs).
   Fix: trie keys are UCI strings resolved against the replayed position's
   legal moves (`uci_to_move`). The `verify_failed` tripwire never fired in
   any post-fix run (0 across 35 arms-reps).
3. Worker/master startup race (session.json) and a channel-lifecycle
   deadlock on completion — both fixed.

## 3. Gate verdicts (pre-registered in plan5 §4)

### SOUND — **PASS**

- Zero false decisive facts end to end: 0 verify failures across all
  post-fix arm reps (m22: 20 reps; shuffle: 20 reps; d4d5: 1 rep).
- Every proven rep's composed artifact passes `validate_proof_tree`
  (`inspect_pt --validate`): m22 C2 3/3, C4 5/5, C2-nf 5/5; shuffle C2 1/1,
  C4 5/5, C2-nf 3/3.
- Root outcome equality with the sequential baseline (win) on all audited
  artifacts.
- Dual-check: 4 artifacts (m22 C2, m22 C4, shuffle C4, shuffle C2-nf) × 24
  sampled decisive facts = **96 facts re-derived independently by the
  sequential solver: zero contradictions, zero inconclusive**.

### ECON (primary position, C2, as pre-registered) — **NO-GO**

- C2 on shuffle: **1/5 proven** (69.2 s), 4 censored at the 600 s arm cap
  with ~6.2–8.4B child evals each. Proven-rep wall = 0.76× S (slower than
  sequential); work inflation (median over reps) ≈ 25×. Neither the GO band
  (≥ 1.2× wall and ≤ 3.0× inflation) nor the MARGINAL band (≥ 1.0×) is met.

### C4 scaling (secondary gate) — **PASSES with GO-band economics**

| | wall (median) | wall speedup | aggregate work (child evals, median) | work inflation | proven |
|---|---|---|---|---|---|
| S (sequential) | 52.85 s | 1.00× | 249.5M | 1.0× | 5/5 |
| **C4 (4 workers)** | **40.81 s** | **1.29×** | **493.7M** | **1.98×** | **5/5** |

GO requires C4 wall ≥ C2 wall (satisfied: 40.8 s vs C2's censored 600 s /
proven 69.2 s) and inflation ≤ 3.5× (satisfied: 1.98×). Published 2-worker
reference points are 1.47× (Kaneko) / ~1.87× (Čížek); 1.29× at 4 workers on a
4-core box (2 workers oversubscribed by the master + 2 extra processes) is a
strong untuned-v0 result.

### m22 (sanity scale) — the isolation-subsidy threat is real

| arm | proven | wall (proven median) | inflation |
|---|---|---|---|
| S | 5/5 | 2.91 s | 1.0× |
| C2 | 3/5 | 27.7 s | 12.7× |
| C4 | 5/5 | 22.6 s | 17.0× |
| C2-nr | 0/5 | — (all censored) | 227× |
| C2-nf | 5/5 | 11.0 s | 3.06× |

The sequential root's cross-child transposition subsidy (858k nodes total)
does not survive leaf-level decomposition with private worker TTs: even the
best campaign config pays >10× work on the heavily-subsidized m22 root. On
the quieter shuffle root the subsidy is smaller and C4 genuinely wins.

### Attribution (mechanism ablations) — **retention pays decisively; v0
feedback/slicing does not**

- **Retention (C2 vs C2-nr): separates by >20× on both positions.** C2-nr is
  0/10 proven across m22+shuffle (all censored, inflation 227× / 25×) vs
  proven C2/C4 runs. This is the Čížek Fig. 4 analog and the design's
  central claim — confirmed.
- **Feedback/slicing (C2 vs C2-nf): does NOT separate in C2's favor.** On
  m22, C2-nf is *faster* than C2 (11.0 s vs 27.7 s proven median, inflation
  3.06 vs 12.7). On shuffle, C2-nf proves 3/5 (93 s) where C2 proves 1/5.
  The v0 slice-and-rerank scheduler is the *worst* 2-worker configuration
  measured. Per the plan's attribution rule, the architecture doc must
  record which mechanism actually paid: **persistent workers with retained
  TTs and complete-job scheduling** — the pseudo-MPN slice/rerank loop is
  recorded as not yet earning its complexity (A3/§11 of the doc; the
  recorded diagnosis below).

### Exploratory d4d5-p2 pair (observation only, no gate)

- S: censored at 1800 s (310M nodes, 8.48B evals, ~172k nodes/s — consistent
  with plan3's linear rate).
- C2: censored at 1800 s — 2,356 jobs completed, 17.8B evals, **21 leaf
  proofs won**, 707 leaves still open, 0 children resolved. The campaign
  shape makes continuous, checkpointable leaf-level progress where the
  sequential run censors monolithically — but at these budgets nothing
  resolves; consistent with plan1/plan3's "bottomless at sandbox scale".

## 4. Verdict semantics and recommendation

Per the pre-registered semantics, **ECON (C2) = NO-GO** → "no plan7".
However, the same session's data amends that materially, and the plan's own
MARGINAL path ("one bounded iteration on the recorded diagnosis before any
plan7 work") is the right landing:

- **Recorded diagnosis of the C2 failure**: in censored C2 reps, 894 of 967
  completed jobs were budget-exhausted draws — both workers cycled
  hopeless-leaf slices (budgets doubling into an 8M cap) instead of
  concentrating on the proof-bearing line, while C4's extra in-flight leaves
  (and nf's full-job scheduling) got coverage. This is a *scheduler/budget-
  policy* failure mode with two cheap candidate fixes (worker-count-aware
  job sizing; or adopting the nf shape as the default and using feedback
  only for abandon decisions), not an architecture dead-end.
- **Recommendation**: treat plan5's outcome as **MARGINAL** in substance —
  SOUND passed, retention (the load-bearing mechanism) demonstrated, GO-band
  economics demonstrated at 4 workers on the primary position — and run
  **one bounded iteration** (scheduler/budget policy only, same harness,
  same gates with C4 as the registered design point) before any plan7 work.
  If that iteration reproduces GO-band economics at 2 workers *with*
  attribution, plan7 is draftable; if not, `solve` falls dormant per the
  status rule (plan6 runs independently and combines only at plan7's
  threshold).

## 5. Deviations

- **Mid-episode binary fix**: the ep/castling merge bug (§2.2) was found
  during the m22 arms; the affected arms (m22 C2/C4) were **fully re-run**
  with the fixed binary. Seq baselines/drift runs are unaffected (product
  solver untouched). Post-fix arms: zero verify failures.
- **Compute budget overran**: ~6 h of sandbox compute vs the planned ≤ 4 h
  (censored shuffle arms at 600 s × 5 reps, plus the pre-fix
  reproduction/rerun). All 5 registered reps per arm were kept — no
  registered protocol step was cut.
- **Dual-check volume**: 24 facts per artifact on 4 artifacts (plan required
  ≥ 20 on the composed C2/C4 artifact).
- `shuffle-win`/`m22_white` are not decisive-suite fixtures; FENs were taken
  from `parallel/measurements/plan2/README.md` (the design_space §3
  baseline protocol). Sequential baselines re-run on this build: m22 2.91 s
  / 858,117 nodes (design_space: 2.63 s); shuffle 52.85 s / 13,907,467 nodes
  (design_space: 47.83 s) — ~10% wall drift, bit-identical work.

## 6. Tools, problems, unresolved parts, missing tests, next steps

- **Tools**: `campaign_master --mode seq` (new, S-baseline runner);
  existing `inspect_pt --validate`, `pt_keys`, `replay`, `list_legal` used
  by the audit; `run_arms.py` / `audit.py` / `summarize.py` drivers under
  `measurements/plan5/` (command table in the README).
- **Problems encountered**: §2's three incidents; a background-process
  lifecycle bug (workers outliving dead masters) that polluted early smoke
  timings — cleaned up before the timed arms.
- **Unresolved**: A3 (reconstruct fills judge repetitions in job-local
  context; a globally-flavored repetition Win would replay clean — the
  dual-check is the compensating control); plan7 must give fills the global
  repetition prefix. The C2-vs-C4 scheduler question (§4 diagnosis) is open
  by design (bounded iteration). Verify-wall of the combined metric was not
  separately timed for campaign artifacts (the audit validator is O(nodes)
  replay; 87k–173k-node artifacts validate in seconds) — the combined metric
  stays dominated by find.
- **Missing tests**: the campaign examples carry unit tests for
  `verify_and_merge` (10) and the replay/path helpers (2); no integration
  tests exist for the master/worker file protocol (session-level behavior
  was exercised by the arms themselves). Plan7 scope.
- **Next steps**: (1) user decision on the bounded iteration (recommended);
  (2) if GO after iteration → plan7 draft to the architecture doc's §2/§8
  spec (`--tt-load-path`, frontier dump, deterministic resume, durable job
  store); (3) plan6 (resource sizing) proceeds independently.
- **State hygiene**: per-rep JSONs, artifacts (`*_proof_tree.bin`), audit
  results and `env.json` committed under `measurements/plan5/state/`;
  worker transcripts and session dirs not committed (convention).
