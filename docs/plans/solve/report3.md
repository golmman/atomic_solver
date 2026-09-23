# Report 3: Quiet-root finishability probe — NOT FINISHABLE at 2 h; ±1-ply substrate GO on the tactical class only

Initiative: `solve`. Executes `plan3.md` (Task A: 2 h finishability
probe on the d4d5 p2 root, arms A1/A2; Task B: ±1-ply substrate
measurement). Runs of 2026-09-22 21:26 – 2026-09-23 02:04 UTC,
strictly sequential as registered (nothing else ran concurrently).
Harness: `measurements/plan3/probe.py` + command table in
`measurements/plan3/README.md`; environment `env.json` (4-core quota
`cpu.max 400000/100000`, 8 GiB cgroup, release build of the unchanged
post-plan2 source at `8e46e52`; **zero source modifications**).

## TL;DR

- **Task A gate: NOT FINISHABLE at the registered 2 h budget.** Both
  arms censor: A1 (128 MB TT) at **1.429 G nodes**, A2 (1 GB TT) at
  **1.211 G nodes**. The step-by-step narrowing scheme's core
  assumption — a single quiet root finishable given a large multiple
  of the plan1 censor budget — is measured false, with the
  **linear-work interpretation** (A1's node rate is 1.02× the 120 s
  plateau rate: clean ~60× wall → ~61× nodes, no end in sight) and the
  memory caveat resolved the negative way (8× TT was *slower*, 0.85×
  A1's rate — no finishability is memory-mediated here).
- **Task B gate: GO** (median avail 23.08% ≥ 5%) — **but on the cheap
  tactical class, not the quiet class**. The pre-registered parent rule
  (shallowest `steer: "pv"` ladder position) structurally cannot sample
  a censored quiet position: across all 8 plan1 ladders, `steer: "pv"`
  ⟺ the 8 s steer completed decisively (26/26). All five parent/child
  pairs are tiny tactical proofs (5–900 nodes, 5–377 visited keys), and
  the child — sitting one ply down the parent's *proven winning line* —
  is wholly contained in the parent's proof (vshare C→P = 100% in all
  five pairs). Real substrate, but in exactly the class where proofs
  are already ≤ 5k nodes and anchors would buy nothing.
- **Joint reading:** every reorganization mechanism (narrowing /
  splitting / anchors) has now lost its last untested universal
  premise. What remains measured-positive lives exclusively in the
  tactical class, where no mechanism is needed. This strengthens
  report2 §9 **option 2 (sharpness-first rescope)**; my recommendation
  as consultant stands, the pivot is the user's.

## 1. Task A — finishability probe (§2 gate)

Root: d4d5 p2 (`rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq
d6 0 2`), plan1's 23.4M-node / 120 s censor point. Both arms
`--timeout 7200 --first-outcome --tt-dump-path`, A2 + `--tt-size 1024`;
stdin DEVNULL, raw captures `logs/arm_a1.*` / `logs/arm_a2.*`, state
`state/arm_a1.json` / `state/arm_a2.json`.

| arm | outcome | nodes | wall (s) | node rate (k/s) | vs 120 s plateau rate | max RSS | snapshot |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A1 (128 MB TT) | censored (draw) | 1,428,643,840 | 7202.5 | 198.4 | **1.02×** | 234 MB | 110 MB: 2.455 M solved + 1.739 M unsolved (58.5% solved) |
| A2 (1 GB TT) | censored (draw) | 1,210,605,568 | 7221.4 | 167.6 | 0.86× | 1.84 GB | 919 MB: 18.155 M solved + 15.400 M unsolved (54.1% solved) |

**Verdict: NOT FINISHABLE at the registered budget** — both arms
censor, so the SPLIT branch never fires and no reconstruction ran
(the `validate: ok` artifact path is unexercised this plan; state
JSONs carry `validate: null`).

Interpretation per the plan's pre-registered dichotomy:

- **A1 is the clean-linear case.** 198.4k nodes/s vs the root's own
  120 s rate (23.4M/120 s = 195.0k/s) and the plan-wide plateau
  (~186k/s): the search converts wall into nodes at a constant rate
  with no degradation and no end in sight. Note the arithmetic: the
  registered 7200 s arm is 60× the 120 s plateau wall (the plan's
  headline says "~10×"; the registered command is authoritative) and
  delivered 61.1× the nodes — exactly linear.
- **A2 rules out memory-mediated finishability.** 8× the TT made the
  rate 0.845× of A1's (mild cache-locality cost, not thrash-and-die)
  and finished *less* work in the same wall. If the hard core were
  TT-thrash-bound at 128 MB, A2 completing would have been the signal;
  the opposite happened.
- Consequence (verbatim from the plan): *no reorganization of runs
  (narrowing, splitting, ordering) can pay at sandbox scale* — this is
  a lower-bound statement about 2 h budgets, not an impossibility
  claim about the position.

## 2. Task B — ±1-ply substrate measurement (§3 gate)

Parents: shallowest plan1 ladder position with `steer: "pv"` per quiet
line; child = parent + first steer-PV move via `examples/replay`
(exact ±1 step, clock semantics preserved by the engine); parent and
child each one 120 s cost run, snapshots kept. 10/10 runs completed
(`state/tb_*.json`, `state/tc_*.json`; every run completed decisively
in ≤ 0.13 s wall, snapshots 0.7–31 KB).

| line | parent (ply) | p nodes | p solved | c nodes | child all keys | ∩ | avail(P→C) | carry(P→C) | vshare(C→P) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| startpos | p10 | 516 | 105 | 388 | 377 | 87 | 23.08% | 82.86% | 100% |
| e4 | p23 | 71 | 11 | 47 | 45 | 10 | 22.22% | 90.91% | 100% |
| d4 | p23 | 71 | 11 | 47 | 45 | 10 | 22.22% | 90.91% | 100% |
| nf3 | p23 | 29 | 6 | 5 | 5 | 5 | 100.00% | 83.33% | 100% |
| d4d5 | p32 | 900 | 190 | 43 | 38 | 37 | 97.37% | 19.47% | 100% |
| **median** | | | | | | | **23.08%** | **83.33%** | **100%** |

**Formal verdict: GO** (median avail 23.08% ≥ 5%). Full data:
`substrate.json`.

### What the GO actually measures — two structural caveats

1. **The selection rule cannot sample the quiet class.** A steer solve
   that censors prints no PV, so `steer: "pv"` ⟺ the 8 s steer solved
   decisively; verified across all 8 plan1 ladders (26 `pv` vs 53
   `capped` positions, zero mixed cases). The plan's caveat ("a
   realistic case, not necessarily the best case") is therefore not
   just a warning but a structural property: this task measured ±1-ply
   substrate *for the tactical class only*. The quiet-class ±1 cell —
   the one plan2's ±2-ply measurement (0.08–0.43%) left open — remains
   unmeasured, and with this rule it is unmeasurable.
2. **The child sits on the parent's proven winning line.** vshare(C→P)
   = 100% in all five pairs: the child's *entire* solved set is a
   subset of the parent's solved set — the parent's proof already
   covers the child's whole search. avail/carry here measure "how much
   of a 5–900-node proof is contained in the proof one ply up", which
   is mechanically near-total and says nothing about whether a
   *censored* quiet parent deposits anything a ±1-ply quiet child
   could consume (plan2 measured that ~0% at ±2 ply).

(e4 p23 and d4 p23 are the same position — the plan2 §Finding-B ladder
convergence — so pairs 2 and 3 are duplicate measurements of one case;
the medians are unchanged either way.)

## 3. What the outcomes mean for the three candidate mechanisms

- **Narrowing (step-by-step prove/disprove, one ply at a time).**
  Premise (a), per-step finishability: measured false for the root at
  2 h (§1); untested for quiet children, but plan1's per-ply plateau
  gives no reason to expect thinning. Premise (b), ±1 substrate:
  measured maximal — but only on the tactical class (§2), where each
  step is already a ≤ 5k-node proof that needs no composing. The
  formal GO does not license the pilot on the population where
  substrate would matter.
- **Budget splitting.** A1's linear rate means nothing is lost to
  overhead by splitting a budget into per-step pieces (no thrash cliff
  at 128 MB; mild, non-fatal degradation at 1 GB) — but nothing is
  gained either: the censor is node-count-bottomless, and no split
  piece finishes sooner than the whole.
- **Anchors (TT deposit/preload).** The only measured-positive
  substrate is the tactical class, where proofs are already free. On
  the quiet class plan2 measured the substrate empty (cross-system and
  ±2-ply); the ±1 quiet cell is open but structurally hard to sample.
  With Task A's NOT-FINISHABLE verdict, an anchor layer has no
  measured population where it would change an outcome.

## 4. Recommendation

**Sharpness-first rescope (report2 §9 option 2).** Both of its
premises are now directly measured: (i) quiet roots do not finish at
60× the plateau wall, at a clean linear work rate, independent of TT
size (§1); (ii) everything cheap and everything substrate-bearing
lives in the tactical class (§2, report1's bimodality, report2's empty
quiet substrate). The rescope needs no new mechanism, no
`--tt-load-path`, no campaign layer — it is a bounded enumeration of
the ≤ 9-ply forced-win class within a fixed opening book, verifiable
end-to-end with the existing search → snapshot → reconstruct →
validate pipeline.

If the user still wants the step-by-step pilot despite §1, it must
first (as a cheap separate measurement plan, not the pilot itself):
re-derive quiet-class ±1 pairs by selecting parents from *censored*
positions (e.g. the deepest capped ladder ply, child = best frontier
child rather than steer PV), and test per-child finishability at 2 h
on one such child. The `--tt-load-path` anchor-preload test flagged by
the Task B GO would need its temporary-instrumentation re-land with
separate user approval — and given §3, I do not recommend spending it
on the quiet class.

## 5. Deviations and problems

- **No product changes**, as registered. Harness-only:
  `measurements/plan3/probe.py` (env / arma / armb / status; `--arm`
  resumability flag so an interrupted 2 h arm can be relaunched) +
  README command table. The snapshot reader is plan2's `ssfp.py`
  imported verbatim.
- **max-RSS via `os.wait4` ru_maxrss** instead of `/usr/bin/time -v`
  (not installed in this container; the plan's "when available"
  fallback), recorded per child process in every state JSON.
- **The plan's "~10× budget" headline vs the registered 7200 s arms**
  (= 60× the 120 s plateau): executed as registered; the linear
  61×-nodes arithmetic is recorded in §1 rather than silently
  reconciled.
- **Task B's degenerate duplicate** (e4 p23 = d4 p23, ladder
  convergence) was anticipated by plan2's Finding B and is reported as
  measured; five pairs ran as registered.
- A large population of pre-existing zombie `atomic_solver` processes
  from the plan1/plan2 sessions was observed in the process table;
  cosmetic, unrelated to this plan, left untouched.

## 6. Missing tests / unexercised paths

- `reconstruct_pt` was never invoked this plan (both Task A arms
  censored); the `validate: ok` artifact contract is unchanged and was
  last exercised in plan1.
- No product code changed → no test-suite impact; nothing to add to
  the fast gate.

## 7. Snapshots pruned (convention)

After analysis, `snaps/*.tt` were pruned to `*.tt.keep` markers
(plan1/plan2 convention). Counts live in the state JSONs
(`tt_solved` / `tt_unsolved` / `tt_bytes`), pair metrics in
`substrate.json`, and the exact commands in the README command table
reproduce content-equivalent snapshots (wall-clock-bounded runs are
never byte-identical). Largest artifacts this plan: A1 110 MB
(4,194,304 keys — 2²² exactly, coincidence), A2 919 MB (33,554,424
keys).
