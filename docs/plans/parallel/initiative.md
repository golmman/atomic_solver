# Initiative: `parallel` — parallel proof search

## Status

**2026-10-14, plan12 executed (groundwork only — stage 0 noise probe; stages
1–3 are GO-branch-only and vacuous) → backlog #7 CLOSED NO-GO; initiative
closed (all items resolved).** The pre-registered H0 noise-floor test ran
mechanically over the committed plan6 campaign data (per-run
helper/coordinator decomposition) plus the research-plan12 corpus salt bands
and two fresh current-binary probes (rem12 6 salts; stress {0,1,5}
robustness). Verdict: **H0 not rejected on both pre-registered cases** —
shuffle-win's t4 bad tails (5.67/5.93/6.33× inflation) sit below every
`max uncensored ratio (3.04×) × overhead` product (≥ 7.50×), with the
decomposition showing a near-constant mechanism work multiplier (~3.7–3.9×
on every t4 run) and the tail explained by mid-band coordinator realization
draws (~72 % of the excess work helper-side); rem12 has no tails and its
3.03× median inflation is the known mechanism overhead (fresh realization
band [0.963, 1.088]); the frequency test cannot veto (sequential stress
tail rate 1/3 uncensored, 4/6 counting the three budget-censored draws,
vs parallel 3/8). **D1 = NO-GO (mechanical): the "trim the bad tail"
premise of #7 is dead — the min-selection it would add is already harvested
by the adopted salt portfolio (zero `src/` risk); the selective re-land,
per-phase accounting, and revert stages never ran.** Out-of-scope finding
(m22, plan6's control): its t3/t4 tails (11.5× / 33.9× inflation,
10–50× typical job counts) DO exceed every noise product (~7.1× max) — a
parallelism-specific inflation component exists on the short-case class
(perturbation / repetition-stall signature), consistent with plan6's own
I4/cap-hit NO-GO record; it does not rescue #7's long-case premise
(S4(m22) = 1.01×). Measurements: `measurements/plan12/`; report:
`report12.md` (closure block inside).

Prior state:

**2026-10-13, plan11 executed → the cooperative-portfolio thread is CLOSED
for good (second strike, per plan11's verdict rule; backlog #12 closed
NO-GO).** The low-tax carrier shipped and removed the solo tax (M1 PASS:
0.995–1.006× vs the ≤ 1.03 ceiling; solo export also proven byte-inert),
and the m20 value channel strengthened (H1: 0.575× evals / 0.601× wall) —
but the pre-registered verdict is reject: M2 fails (stress 1.141×, m22
1.110× vs ≤ 1.06×) and H2 fails on stress + dec10. Diagnosis (report11):
the M2 excess is the 4-way contention floor of the environment itself
(fresh facts-free A0 control: pure R0 at 1.24–1.25× solo on dec13), and
the surviving cost channel is TT pollution (imports perturbing winning
basins). No plan12; #7 (any-worker-can-finish SPDFPN) is the only open
item.

Prior state:

**2026-10-13, owner decision — cooperative-portfolio thread reopened for
plan11 (drafted, docs only, no code).** Report10's next-steps option 2 is
taken: the owner weights the m20 value channel (0.66× evals / 0.82× wall,
winner below its own solo basin minimum) higher than the uniform tax.
`plan11.md` pre-registers a **low-tax facts carrier** — key-only emission
(dedicated `FactExporter` hook, no path clone, no channel, no writer
thread), chunk-boundary handoff (threadless importer reading sibling files
directly at the existing hook, bounded 1 MiB/call), and a fixed admission
filter (`PATH_LEN_CAP = 24`, near-root emission) — reusing plan10's
soundness surface, harness, and race-pairing gate adaptation verbatim.
The pre-registered mechanism hypothesis quantifies report10's wall-tax
attribution (M1 solo ≤ 1.03× vs plan10's 1.19–1.25×; M2 in-race R0 ≤ 1.06×
vs plan10's 1.38×); H1/H2 keep plan10's 0.9×/1.1× lines. No escalation
arm: any reject or defer closes the cooperative-portfolio thread for good.
Backlog item #12 row added at execution start; #6 stays closed NO-GO.
Prior state:

**2026-10-13, plan10 executed → item #6 CLOSED NO-GO (rejected by its own
pre-registered rule).** The cooperative portfolio shipped as code
(`--facts-out` / `--facts-in`, `src/facts/` codec + threads, the
one-method `search::dfpn::FactImporter` hook drained between work chunks;
drift gate clean: quick 59/59 eval-identical, m22/shuffle-win stdout
byte-identical, TT-snapshot sha unchanged; fmt/clippy/doc green, `make
test` green) and the campaign ran fully: stage-1 paired export-inertness
exact, stage-2 canary clean on m22+stress (A2 fallback never triggered),
stage-3 5-round × 6-case A0/A1 campaign. Soundness/fidelity all green
(zero decisive-outcome conflicts in 58 races, zero CRC failures, pure-R0
evals reproduce fresh same-binary solo anchors bit-for-bit; the dfpn
report10 destabilization record does **not** extend to exact-key
 cross-worker sharing). Verdict **reject**: H2 failed on 5/6 cases via a
uniform facts-plumbing wall tax (1.2–1.4× `W_first` at ≈1.0×
`evals_first`) while H1 fired exactly where pre-registered (m20_white
0.66× evals / 0.82× wall — cross-racer subtree dedup below the winner's
own solo basin minimum) but with a 1-in-5 winnerless tail (m20 A1 r3
all-censored). Value validated, carrier too expensive; a cheaper facts
shape (key-only emission, batched imports) is draftable as a new plan if
the owner weights the m20 channel higher than the uniform tax. Findings:
plan12/plan8 anchors are stale for the current binary (re-baselined onto
fresh paired anchors; determinism intact). Measurements:
`measurements/plan10/`; report: `report10.md`.

**2026-10-12, plan9 executed → item #11 CLOSED (shipped).**
`examples/portfolio` landed exactly per plan (zero `src/` diff, asserted
stage 0 and re-asserted in `report9.md`): Rust example driver, default
racer set {0, 2, 3, 5}, fixed racer flags `--first-outcome
--outcome-only`, stop-when-proven kill policy (no grace period — the
m20 validation race cost ≈ 248 core-s vs 747 with plan8's 240 s grace),
plan8 censoring discipline in code, separable `RaceTracker` state machine
as the #6/#7 harness skeleton. Gates: fmt/clippy green, `make test` green
(9 unit tests in the example + 2 fast-tier smoke tests in
tests/portfolio_smoke.rs). Fidelity: dec10 smoke reproduces the plan8
anchor exactly (winner salt 2, 2,918,636 evals); m20_white operator
validation reproduces its anchor exactly (winner salt 5, 791,401,553
evals, exit 0, wall 64.3 s). Report: `report9.md`; measurements:
`measurements/plan9/`. The option-3 door (`--race` product flag) stays
explicitly open only behind a #6/#7 scalable mechanism.

**2026-10-12, owner ship-shape decision: option 2 (`examples/portfolio`
driver); backlog item #11 opened (plan9 drafted).** Rationale in
`plan9.md` §Decision record: option 3 (`--race` product flag) rejected for
now — the salt portfolio is a PoC-scale lever (fixed pre-registered racer
set tuned on the gate cases, linear CPU/RAM cost, no work sharing), and a
product surface would freeze that shape in just before the cooperative
mechanisms (#6/#7) that own actual scaling; option 1 alone leaves the
measured win behind manual steps. The driver is also the intended harness
skeleton for #6/#7. Python driver rejected in favor of a Rust example
(examples/ convention, consistency, tier-tested).

**2026-10-11 (later the same day), plan8 executed → ADOPT; backlog #5
closed WON.** The pre-registered verdict fired mechanically: **H1 pass**
(the adopt trigger — m20_white converts: every race had ≥ 1 uncensored
decisive racer, `W_first` median 95.89 s ≤ 160 s gate; the shipped salt-0
run is budget-censored at 2 B evals / ~313 s with no answer, the portfolio
proves the win at 791,401,553 evals in every race), **H2 pass** (dec10
`W_first` 0.768× ≤ 0.80×; m23 clause recorded-not-gated per the fixed
raced table), **H3 pass** (stress 1.046× / m22 1.063× / dec13 1.054×,
all ≤ 1.10× — measured 4-way contention tax ≤ 6.3 %), **H4 pass** (5/5
stage-0 identity; 63/63 decisive racers reproduce their plan12/stage-2
anchors bit-for-bit under concurrency; zero outcome conflicts; censoring
discipline held — 3 censored, 6 wall-killed recorded, evals excluded).
The frozen stage-1 pre-computation held exactly: `evals_first` = the
predicted basin minimum on every raced case, and the min-evals racer
finished first in all 18 races (wall tracks evals). Out-of-sample rem12
(not salt-invariant: 6 basins) confirmed independently — 0.889× wall,
0.856× evals. Cost: ~747 core-s per m20 race (the first answer ever on
that case) vs 313 core-s that yield nothing shipped; ~3.9× CPU family-
wide for ≤ 6 % wall regression elsewhere. Ship-shape decision space
(operator pattern vs `examples/portfolio` driver vs product flag)
recorded in `report8.md` §Ship-shape — not implemented; owner's call.
#6/#7 sequencing unblocked: the draw-luck regime is confirmed at gate
grade. Measurements: `measurements/plan8/`; report: `report8.md`.

**2026-10-11, owner decision — initiative reopened: salt-seeded portfolio
at gate grade (plan8, in execution).** The re-examination addendum's
sequencing resolves in favor of backlog **#5** (diversified portfolio
re-run): all config-diversity arms are closed at gate grade (research
plan13/17/18), the single-core competitor #20 is closed no-go (research
plan20), and the shipped `--salt` option (research plan11/12) makes a
zero-product-code salt portfolio possible. Plan8's key pre-computation: a
salt racer's trajectory is concurrency-independent (private TT, no shared
state, deterministic per (case, salt)), so the portfolio's evals-space
result is min-over-racer-salts over the committed plan12 rollout —
pointwise ≤ the shipped salt-0 run on every case, with the predicted win
surface exactly the priority-1 gap the old gates missed: **m20_white is a
shipped 2 B-budget timeout that racers s3/s5 convert into a proven win at
≤ 0.86 B evals**, plus dec10 at 0.685×. What plan8 measures is what is
genuinely unmeasured: 4-way-contention wall reality, `W_first` on the
conversion case, out-of-sample rem12, and fidelity (per-racer plan12
identity in-race). Verdict under the pinned gate v1.0 with a declared
portfolio-shaped comparison adaptation (no per-salt pairing exists).
Gate: `plan8.md`.

**2026-10-09, re-examination note (docs + read-only probes; status
unchanged — closed).** `reexamination.md` re-reads the no-go record and
finds that it over-reaches in four places: (1) plan7 never tested a
diversified portfolio — under `--first-outcome` `--refine-cap` is inert,
so P0/P1/P2 were the identical computation, and P3's ε = 0.25 was already
the measured-worst value on both motivating cases; a re-run with
ε ∈ {0.125, 0.375, 0.5, 1.0} measured W_first = 0.63× (shuffle-win) /
0.75× (rem12, out of sample) / 0.91× (m22), passing plan7's own H1/H2
(single race per case, selection-bias caveat on ε = 0.5); (2) SPDFPN was
only measured at the capacity-starved 128 MB TT and with a
coordinator-only-finish shape; (3) the ~5 % sequential tax was avoidable
(monomorphized TT backend or no shared TT); (4) the gates measured median
speedup on solving cases rather than timeouts converted on the deep tier.
Ranked follow-up ideas (diversified/cooperative portfolio, SPDFPN TT-size
diagnostic, Kaneko root threads, TT prefetch) are recorded there; nothing
is re-opened by the note — a re-open is a new plan with its own
pre-registration. Evidence: `measurements/reexam/`; report7 addendum.

**2026-10-05, plan7 executed → NO-GO confirmed; initiative re-closed.**
Plan7 tested the last unmeasured mechanism shape — a process-level portfolio
(K = 4 independent *sequential* solver processes racing with diverse
`--refine-cap`/`--epsilon` configs, take-first decisive; zero product-code
changes) — to trim plan6's observed shuffle-win t4 bad-trajectory tail.
Verdict (mechanical, pre-registered in `plan7.md`): **H1 fails — shuffle-win
W_first median = 0.986× the baseline median (gate ≤ 0.75×)**; H2 pass (m22
0.992×, rem12 0.967×), H3 pass (120/120 decisive `win`, zero cap-hits, zero
panics). The load-bearing diagnosis: the solver is **deterministic per
(FEN, config)** — each racer's trajectory is fixed, so the 8 races are 8
replays of the same race, and the plan6 t4 bimodality (20–39 s vs 90–100 s)
**does not exist in sequential single-process runs** (40 racer runs + 16
baselines, all within ~1% of the case median): it was an artifact of the
SPDFPN shared-TT mechanism itself, not a harvestable path-dependence. Cost:
~4.4× CPU core-seconds for −1.4% wall. The "ship an `examples/portfolio`
driver" follow-up is answered **do-not-ship**. `report7.md`,
`measurements/plan7/`.

Opened 2026-09-21 as the consolidated home for the parallelization effort,
absorbing `conversion` backlog #4 and `lean` backlog #2 (joint ownership
split across two initiatives until now). The next plan number is **plan1**.

**2026-10-03, owner decision — gate re-registered; initiative reopened for
plan4b/plan5.** The plan4 kill verdict stands unchanged as the measured
record; the owner consciously accepts the measured TT-concurrency tax
(+4.4 % / +6.0 % hard-class wall) as the entry fee for the A-stage — the
only remaining multiplicative lever (all others measured no-go). Three
conditions are pre-registered with the acceptance:

1. **Conditional acceptance (revert-if-missed).** The tax is accepted
   *only while* plan5 proves out: if the SPDFPN prototype misses its GO
   bands (≥ 2.0×/4 threads, work inflation ≤ 2×, zero soundness
   violations), the refactor is reverted again and the tax goes with it.
2. **Explicit gate, not a waiver.** The plan4 gate (≤ +2 % wall) is
   replaced by a hard budget: **≤ +7 % median hard-class wall** (m22 /
   shuffle-win first-outcome, vs the unsharded baseline) with the
   byte-identity surface unchanged (quick-suite `child_evals`, stdout,
   snapshot bytes). Future TT work may not erode the sequential path
   silently beyond this budget.
3. **Measurement honesty.** The reference container exposes 4 CPUs: wall
   measurements at 2–4 threads are real; N ≥ 8 are simulated only. The
   conservative band at 4 threads (~1.8×) sits at the GO-band edge — a
   marginal result is a realistic outcome, not a surprise.

**2026-10-04, owner directive — plan6 executed → NO-GO confirmed, initiative
closed.** Plan6 re-landed the stage-2 mechanism verbatim (b714ef6 + W delta),
locked W = 20 000 by the pre-registered long-class confirmation (the plan5b
m22 winner 10 000 does not hold on shuffle-win: 23.3 s vs 29.4/29.7 s
medians), added the mid-length case **rem12** (17.1 s sequential, selected
from `tests/fixtures/decisive_remaining.txt`), and re-ran the campaign at
**8 interleaved reps** × N ∈ {2, 3, 4} on m22 + rem12 + shuffle-win.
Verdict against the re-registered gate (mechanical, `plan6.md`):
- **H1 fails on both conjuncts**: shuffle-win S4 = 1.65× (< 2.0×) and
  I4 = 2.22× (> 1.5×) — the plan5b shuffle-win promise (2.68×/1.41×) was a
  5-rep artifact of a bimodal run distribution (5 of 8 runs healthy at
  20–39 s, 3 of 8 in a bad-trajectory tail at 90–100 s / 5.7–6.3× eval
  inflation; the 5-rep medians had sampled the good tail).
- **Mid case fails twice**: rem12 S4 = 1.20× (< 1.5×), I4 = 3.03× (> 2× —
  the plan's explicit NO-GO clause).
- **Cap-hit gate fails**: C = 3/8 = 0.375 > 1/8 (shuffle-win t2).
- Soundness stayed clean throughout: 64/64 decisive-run agreement (every
  run `win`), 8 cap-hit timeouts recorded separately, zero panics.
- H2 (length trend) is directionally real — S4 = 1.01× (m22) → 1.20×
  (rem12) → 1.65× (shuffle-win), Spearman ρ = 1.0 — but the achievable
  level at honest rep counts sits far below every GO threshold.
Condition-2 revert executed identically to `report5.md`: 0-line code diff
vs pre-plan4b, `make test`/fmt/clippy green, quick suite 59/59, all three
byte-identity hashes identical to the plan4 record. **Backlog #4 closed
NO-GO (strengthened record: the no-go survives a second, larger-
sample, mid-case-extended campaign with the W re-swept per
pre-registration).** The parallelism space as scoped is measured out end
to end (plan2 option C, lean plan7, SPDFPN stage 2 × 2 campaigns);
re-open levers (per-phase accounting, TT replacement scoring, helper
caps) remain recorded in `report5.md`/`report6.md` — any re-open now
needs a *mechanism* lever, not more measurement.

**2026-10-04, owner directive — initiative re-opened for plan6 (in
execution).** After reviewing plan5b's numbers the owner directed one more
measurement round before a final decision: shuffle-win is promising (S4 =
2.68×, I4 = 1.41×) while the very short m22 case (S4 = 1.79×, I4 = 2.06×)
is a weak 5-rep median. Plan6 re-lands the stage-2 mechanism verbatim
(b714ef6 + W = 10 000) and executes a **new pre-registration** (the plan5b
bands are closed history, not renegotiated): length-threshold hypothesis
H, a mid-length case, 8-rep medians, cap-hit rate as a first-class gate
item, and a scoped ship shape (opt-in `--threads N`, documented "long
solves only"). Gate: `plan6.md`; verdict and closure in `report6.md`.

Next: **plan4b done (stage 1b re-landed, tax pinned +5.36 %/+4.61 % vs
the ≤ 7 % budget, `report4b.md`)**; **plan5 done (stage 2a mechanism built,
smoke sound but 1.42×/4 threads — `report5a.md`)**; **plan5b done (stage 2b
campaign: W locked at 10 000, S4 = 1.79×/m22, I4 = 2.06×/m22 → NO-GO per the
pre-registered bands; condition-1 revert executed — stage 1b + 2a reverted,
byte-identical sequential solver restored, tax refunded,
`report5.md`)**. **2026-10-04: backlog #4 closed NO-GO; initiative closed —
the parallelism space as scoped is measured out end to end** (option C
plan2, deterministic lean plan7, SPDFPN stage 2; see `report5.md` next
steps for the re-open levers if the owner ever revisits option A).

**2026-10-03, plan4 kill point fired — initiative closed (backlog #4 NO-GO).**
The inert TT-concurrency refactor measured +6.0 % (m22) / +4.4 %
(shuffle-win) first-outcome wall vs the pre-registered ≤ +2 % gate —
with zero deterministic drift — and no in-scope mitigation recovered it
(profiled; three variants tried; seqlock redesign scoped and rejected).
The refactor was reverted; the A-stage dies at the cheap kill point as
pre-registered. Details: `report4.md`, `measurements/plan4/`.

**2026-10-02, premise change — initiative reopened for option A.** The
product consumer now accepts nondeterministic parallel runs
(nondeterminism confined to *which* valid proof wins, never a false
decisive outcome). This satisfies the precondition that plan2's NO-GO
deliberately deferred: option A (thread-level shared-TT parallel DF-PN)
was gated on mining Pawlewicz & Hayward 2014, now mined (plan3,
`research_spdfpn.md`): mechanism fully specified (W-threshold
interruptible jobs, virtual win/loss steering over a per-ply virtual TT,
shared-TT discipline), serial base identical to ours (dfpn 1+ε),
measured 0.74 efficiency /16 threads with work inflation ≤1.40× — the
exact gap that killed option C. The option-C no-go and the lean plan7
deterministic no-go stand untouched. Next: backlog #4 (plan4 inert
TT-concurrency refactor = cheap kill point; plan5 prototype with
pre-registered GO bands).

**2026-09-21, plan2: architecture selection closed as measured NO-GO.**
The option space is now closed by measurement on both sides: the
shared-state architectures (A/B/D) are excluded by soundness/economics
(plan1 mining; `design_space.md` §1), and the only constraint-satisfying
shape — option C, a harness over unmodified solver processes — measured
**0.27× wall speedup** (a 3.7× slowdown) at 4 workers on m22 with
15.5× work inflation, far outside the pre-registered GO/MARGINAL bands
(`design_space.md` §6, `measurements/plan2/`). The mechanism is
structural: per-child process isolation forfeits the root search's
cross-child transposition sharing and best-first proof interleaving
(858 k nodes for the whole root vs 8.59 M for the winning child alone).
No opt-in parallel mode ships; the initiative moves toward closure
(backlog #3 never unblocks). The Pawlewicz & Hayward 2014 bibliography
entry stays **Open** for a hypothetical future A-stage.

The recorded evidence stands and is not reopened: deterministic parallelism
is a **measured no-go** (`lean` plan7 spike — sibling-parallel ceiling
1.47–1.48×, below the ~1.5× bar; the AND early-exit is already harvested
sequentially). What this initiative opens is the remaining space:
opt-in parallelism whose nondeterminism is bounded and documented, and
architectures that do **not** rely on a shared mutable TT at all.
`lean` report7's reopen triggers are consciously renegotiated by this
opening: trigger 2 (algorithmic levers exhausted without ~2× work
reduction) is now satisfied — `dfpn` is dormant and `conversion` #6/#7
closed as measured no-gos, leaving parallelism as the only multiplicative
lever on the roadmap. Trigger 1 (a consumer accepting nondeterministic
runs) is deferred to the design note: whether an opt-in nondeterministic
mode ships at all is a plan1 output, not a precondition.

**2026-10-26:** the rejected plan10/plan11 carrier code (`src/facts/`, the
`search::dfpn` import/export hooks, and the `--facts-out` / `--facts-in`
flags) was removed by `cleanup` plan6; the plan10/plan11 reports and
measurements remain the historical record.

## Motivation

The solver is sequential and single-threaded end to end. All other lever
families are measured out at the diagnostic level (`research`/`lean`/
`conversion` no-go records; `research/structural_floor.md` §9). Parallelism
is the one lever with a multiplicative wall-time ceiling (2–8× on
multicore hardware), and the only one that does not touch DF-PN threshold
arithmetic or TT cacheability semantics — its risks are concurrency and
GHI soundness instead.

Prior mining: Kaneko AAAI-10 is fully extracted
(`dfpn/research_parallel.md`, implementation guide with virtual pn/dn
congestion terms, Mark/Unmark agent tracking, shared stop-set, per-node TT
locks; ~3.6× on 8 threads in the paper). Its 15-year-old scaling
assumptions are superseded by the 2025 massively-parallel PNS work and
JLPNS — both unmined (see backlog #1).

## Goal

Make wall time on hard searches scale with available cores without ever
returning a false decisive outcome, by (a) mining the remaining parallel-PNS
literature, (b) selecting an architecture that fits this solver's
soundness contracts, and (c) staged implementation behind an explicit
opt-in. Priorities follow AGENTS.md: correctness first, then performance,
memory, maintainability.

## Design constraints (carried over, normative)

1. **Sequential path stays bit-identical.** Any parallel mode must be a
   strict opt-in (`--threads N` or equivalent); with the option off, every
   stdout byte, TT trajectory, and `child_evals` count matches today.
   The lean drift protocol applies unchanged to the sequential path.
2. **No false decisive outcomes.** Any candidate must state why it can
   never prove a non-proof (same bar as `conversion`'s non-goal).
   Cross-worker TT reuse makes the `dfpn` journal-GHI contract
   (plan10 hazard class; `dfpn/research_ghi_journal.md`) load-bearing:
   repetition-dependent results must never be shared/cached across worker
   contexts unless the reuse argument survives cross-path composition.
3. **Budget determinism contract.** `child_eval_budget` exhaustion →
   `Draw` + `ExitReason::BudgetExhausted` is a documented deterministic
   surface; a parallel mode must either preserve it or explicitly
   re-document it as nondeterministic under `--threads > 1`.
4. **RAM accounting.** The search CLI's "RAM = TT only" contract must be
   restated, not silently broken, if an architecture multiplies TTs
   (e.g. per-process private tables).

## Architecture options (the design-space note, backlog #2)

Not decided yet — plan1/plans decide. The honest option set:

- **A. Thread-level shared-TT lazy-SMP** (`--threads N`): workers run
  DF-PN over one locked/sharded TT. Kaneko's virtual-pn congestion terms
  are the published cooperation protocol. Highest expected speedup on
  paper, highest engineering risk, makes constraint 2 load-bearing, and
  the CLI surface is exactly the parked `--threads N`.
- **B. Process-level portfolio**: N independent solver processes (each
  today's sequential solver, own TT) racing with different parameterizations
  (ε, ordering tweaks, TT size); first decisive result wins. No shared
  mutable state — soundness is inherited per worker, constraint 2 vacuous.
  Nondeterminism confined to *which* valid proof wins, never a false one.
  Memory N×TT; buildable almost entirely outside `src/` (examples/
  harness). Plausible precisely because trajectory chaos makes diverse
  parameterizations fail independently (`conversion` report8: dec01 5.7M
  evals at ε=0.125 vs 210M unfinished at ε=0.375).
- **C. Root/child split, per-worker TTs**: distribute the root's children
  across workers. AND-side (all children needed) combines deterministically;
  OR-side races children (nondeterministic which proof is found). Clean for
  the `find_winning_child` / EGTB workloads; work-wasteful vs the
  sequential early-exit for single-root solving.
- **D. Two-level/shared-worker protocols (Čížek 2025)**: the massively
  parallel PNS design; subject of the reading round before any comparison
  is meaningful.

**Cross-paper comparison (plan1 reading round, 2026-09-21;**
`research_cizek2025.md`, `research_jlpns.md`, `research_solrex.md**):**
All three papers are silent on repetitions under concurrency (Sprouts /
Breakthrough / Hex are position-monotone) — constraint 2 has no published
soundness argument anywhere in this literature; the journal contract
(`dfpn/research_ghi_journal.md` §5) stays load-bearing for every option.
What the evidence does settle: (1) **C is the strongest survivor.** The
only near-linear small-pool (4–16 core) measurements in the literature
(Čížek Table 1: 4.3×/4, 16.2×/16; PPN₂: 7.2×/16) come from job-level
decomposition over *persistent private-state workers with no shared TT*
— in Čížek's own data the shared-TT second level (`threads > 1`) only
engages at ≥128 cores, and PPN₂'s clients never share state at all. This
is exactly our option C's shape, implementable as a harness over
unmodified solver processes (sequential path bit-identical by
construction; constraints 1/2 vacuous at the worker level; the master's
decisive-fact combination is rule-sound, worker Draws consumed as
unproven). (2) **B is a degenerate adjunct, not the main lever**: JLPNS/
PPN₂ is *not* a portfolio — central coordination and re-direction are
where its speedup comes from, and Čížek's ablation shows discarding
worker state between jobs is catastrophic (PNS-PNS falls below
sequential). (3) **A is real but second**: Solrex/SPDFPN delivers the
best few-thread shared-TT figure published (≈5.2×/4 threads, superlinear
caveat; 11.8×/16 per Čížek's citation), but its mechanism lives in an
unmined source (Pawlewicz & Hayward 2014), no work-inflation figure is
reported, and sharing a mutable TT makes the journal contract bind at
every TT site (L–XL effort: the inert TT-concurrency refactor first).
(4) **D is dead as a distinct option**: Čížek's master-level shared
database — its biggest scaling lever — carries path-independent key
results (Grundy numbers); our only rule-path-independent class (decisive
Win/Loss) is banned from cross-worker reuse (plan10 non-goal) and our
draws are path-dependent, so the database has no safe payload here; the
rest of D reduces to C (first level) plus A (second level, idle below
128 cores). Recommended shape for plan 2 to test: **C first (harness,
S–M effort), A behind it as an opt-in second stage**, B only as a cheap
parallel adjunct. **Plan-2 sizing spike, in order**: (i) worker
tail/utilization and `child_evals` inflation (vs sequential) for C at
4–16 workers on m22/shuffle-win — the OR-early-exit tail is the
unmeasured risk; (ii) master-side decisive-composition agreement (does
an isolated worker solve of each root child reproduce the sequential
child outcome — extends the two-solves-in-one-process property test);
(iii) RAM at N×TT (constraint 4 restatement); (iv) only if A advances:
mine SPDFPN 2014 and measure a 2-thread drift-safe prototype.

## Backlog

| # | Item | Mechanism | Potential | Affects | Effort | Status |
|---|------|-----------|-----------|---------|--------|--------|
| 1 | **Reading round: parallel PNS (2025-era)** | Mine the three open bibliography entries into `research_*.md` here: (a) Čížek, Balko, Schmid 2025 (*Massively Parallel Proof-Number Search*, arXiv:2511.10339 — two-level parallelization + shared worker info, 333× on 1024 cores); (b) Saffidine, Jouandeau, Cazenave 2011 (JLPNS, ACG-13); (c) Young, Hayward 2016 (*A Reverse Hex Solver*, scalable parallel DF-PN, Solrex). Extract per paper: cooperation protocol, TT sharing/sharding model, GHI/repetition handling under concurrency, scaling regime (where the gains saturate on 4–16 cores, not 1024) | information (feeds #2) | — | S | **mined 2026-09-21** (plan1): `research_cizek2025.md`, `research_jlpns.md`, `research_solrex.md` (+ vendored PDFs); all three silent on repetitions under concurrency (monotone domains) — the finding; small-pool evidence favors C over A; D's shared-DB layer has no safe payload here |
| 2 | **Design-space note: architecture selection** | Weigh options A–D above against the design constraints; include the API/consumer story (opt-in `--threads N` vs a process-portfolio harness vs both). Sizing spike allowed per lean's cadence (temporary instrumentation, reverted) | selects the 2–8× lever's shape | — | M | **closed 2026-09-21 (plan2): measured NO-GO** — `design_space.md` + `measurements/plan2/`: option C (root-child race over unmodified processes) measured 0.27× wall (3.7× slowdown) at N=4 on m22, 15.5× work inflation, zero soundness violations; deficit structural (forfeited cross-child TT sharing/interleaving); ship-shape verdict recorded (harness-only, moot); no parallel mode ships |
| 3 | **Staged implementation** | Gated on #2 GO; one lever per plan; sequential drift protocol green at every stage; parallel mode documented as nondeterministic (or not shipped, per #2) | the actual speedup | wall | L–XL | **closed 2026-09-21** — unblocking condition (#2 GO) failed on the pre-registered rules; no staged implementation exists to stage |
| 4 | **A-stage: SPDFPN-style shared-TT parallel DF-PN** | Reopened by the 2026-10-02 nondeterminism premise; reference design = `research_spdfpn.md`. Stage 1 (plan4/plan4b): inert TT-concurrency refactor — sharded TT behind interior-mut locking, byte-identical N=1 behavior (gate re-registered 2026-10-03: ≤ +7 % median hard-class wall, byte-identity unchanged). Stage 2 (plan5/plan5b): `--threads N` prototype — W-threshold + same-child resume clause, virtual TT + job lock + `TRYRUNJOB`, thread-local repetition cache, serialized ProofEvents; measured on the hard class (wall speedup, work inflation, outcome agreement) with pre-registered GO bands; **tax acceptance + refactor revert-if-missed** | 2–8× wall on hard solves (expected ≈ 2.9–3.2× at 4 threads; conservative band ~1.8× at the GO edge) | wall | L–XL | **closed 2026-10-04 NO-GO (plan6, second campaign; `report6.md`)**: plan6 re-landed stage 1b+2a verbatim, locked W = 20 000 by the pre-registered long-class confirmation, added mid case rem12 (17.1 s seq), and re-ran the campaign at 8 interleaved reps × N ∈ {2,3,4}: **H1 fails both conjuncts (shuffle-win S4 = 1.65× < 2.0, I4 = 2.22× > 1.5 — the plan5b 2.68× was a 5-rep artifact of a bimodal run distribution)**, rem12 fails twice (S4 = 1.20× < 1.5, I4 = 3.03× > 2), cap-hit gate fails (C = 3/8 > 1/8); soundness clean (64/64 decisive `win`, zero panics); length trend directionally real (ρ = 1.0) but far below every GO threshold. Condition-2 revert executed (0-line diff vs pre-plan4b, all hashes identical). Prior state: closed 2026-10-04 NO-GO (plan5b; `report5.md`) — re-opened same day by owner directive; see `plan6.md` for the second pre-registration: stage 2b campaign (W sweep locked 10 000 — the landed 20 000 measured 3× worse at median; 5 interleaved reps × N ∈ {2,3,4} on m22 + shuffle-win): outcome agreement 29/29 decisive `win`, zero panics; **S4 = 1.79× (m22) / 2.68× (shuffle-win), I4 = 2.06× (m22) / 1.41× (shuffle-win)** → m22 misses both GO conjuncts, I4 > 2× fires NO-GO. **Condition-1 revert executed**: stage 1b + 2a reverted (0-line code diff vs pre-plan4b, `make test` green, quick suite 59/59, snapshot sha identical) — the tax is refunded. Mechanism preserved in history (`b714ef6` + W delta) and `measurements/plan4/`; re-open levers in `report5.md`. Measurements: `measurements/plan4/`, `plan4b/`, `plan5/` |
| 5 | **Diversified portfolio re-run at gate grade** | Re-test plan7's downgraded-to-unjudged question under the pinned gate v1.0 with the only surviving diversity source: K concurrent sequential racers at different `--salt` values, default-inclusive, take-first decisive. Config arms (ε, history/killer, clock-budget reuse) are closed at gate grade (research plan13/17/18) and are not racer arms. Evals-space result pre-computable from plan12 data (racers are concurrency-independent); the unmeasured surface is contention wall, `W_first` on the m20_white timeout conversion, and out-of-sample rem12 | priority-1 timeout→proof conversions on the deep tier; ~1.0× elsewhere (pointwise dominance) | none (zero product code; `--salt` is shipped) | S | **closed 2026-10-11 ADOPT (plan8; `report8.md`)** — H1 pass (m20_white shipped-timeout → proven win at 791.4 M evals, `W_first` median 95.89 s, conversion in 3/3 races), H2 pass (dec10 0.768×), H3 pass (≤ 1.063× everywhere), H4 pass (5/5 identity, 63/63 in-race anchor fidelity, zero conflicts); prediction held exactly (evals_first = predicted basin min, wall tracked evals in 18/18 races); out-of-sample rem12 won independently (0.889×). Ship-shape space recorded, not implemented (owner decision); #6/#7 sequencing unblocked |
| 6 | **Cooperative portfolio (ManySAT shape)** | Private per-worker TTs + an append-only shared solved-facts table; one pure default racer never consumes it, so the worst case is the sequential wall. Solved entries are path-independent and already reused across paths within a run; keep the `best_move_repeats_path` guard; same root and run only (not the plan10 cross-solve store). Risk (reexamination idea B): extra solved facts can destabilize DF-PN (`dfpn` report10/11) — read under the draw-luck regime plan8 establishes | could harvest cross-racer facts, not just min-of-draws | M (product code) | M | **closed 2026-10-13 NO-GO (plan10; `report10.md`)** — mechanism shipped (`--facts-out`/`--facts-in`, `src/facts/`, `search::dfpn::FactImporter` hook; drift gate clean, `make test` green) and stayed sound everywhere (zero conflicts/panics/CRC failures in 58 races, canary clean, pure-R0 evals fidelity bit-exact; the dfpn report10 collapse record does NOT extend to exact-key cross-worker sharing). Pre-registered verdict **reject**: H2 failed on 5/6 cases — a uniform facts-plumbing wall tax (1.2–1.4× `W_first` at ≈1.0× `evals_first`; emission path-clone + per-racer writer/reader threads, +15–25% wall even solo) at the stop-when-proven winner's wall, not a search regression; H1 fired exactly where predicted (m20_white 0.66× evals / 0.82× wall — the winner finished below its own solo basin minimum via near-root subtree dedup) but with a 1-in-5 winnerless tail (m20 A1 r3 all-censored). A cheaper facts shape (key-only emission, batched imports) is draftable as a new plan; A2 (Loss-only) fallback never triggered |
| 7 | **Any-worker-can-finish SPDFPN** | The reexamination idea C successor: the TT-capacity diagnostic premise is dead (bigger TT regresses sequentially — `research/reexamination.md` §4), so only the architecture point survives, re-motivated from "avoid perturbation variance" to "harvest draw luck with min-selection" (every worker can prove the root, first finish wins). Prerequisite reading: plan6's bad-trajectory tails (5.7–6.3×) sit inside the sequential single-config salt noise band — re-read under plan8's distributions before building | trims the SPDFPN bad tail, if it was not a noise-floor artifact | re-land (`b714ef6` + W delta) | M–L | **closed 2026-10-14 NO-GO (plan12 stage-0 noise probe; `report12.md`)** — H0 (tails = realization draws under the measured parallel overhead) not rejected on both pre-registered cases (magnitude: shuffle-win tails 5.67–6.33× below every ratio×overhead product ≥ 7.50×; rem12 has no tails, its 3.03× median inflation = the known ~3.8× mechanism overhead); frequency cannot veto; the re-land/accounting stages never ran (GO-branch-only). Out-of-scope m22 finding recorded: a parallelism-specific inflation component exists on the short control (11.5×/33.9× tails vs ~7.1× max noise product) — consistent with plan6's I4/cap-hit record, not a #7 premise |
| 8 | **Kaneko root threads with virtual pn congestion** | Root-started workers carrying their own in-call path state (`dfpn/research_parallel.md`, never implemented) sidestep the TT-only TRYRUNJOB descent blindness in repetition-heavy regions (first-player-loss GHI shortcut leaves nothing to guide on) | avoids the §2 repetition-region stall that stalled the pure SPDFPN pool | L | L | **open, fringe** — cross-referenced as research backlog #22(b) |
| 9 | **Sequential-tax removal enabler** | Make `Search` generic over a TT backend (sequential build monomorphizes to today's unsynchronized table, byte-identical by construction) or do not share the TT at all (private per-worker TTs + small shared solved-facts table). Removes owner condition 2 (the ≤ +7 % TT-concurrency tax budget) from any future shared-TT pre-registration (reexamination §3) | entry-fee elimination, not a mechanism | none | M | **open** — only worth scheduling if #6/#7 produce a GO |
| 10 | **Fringe: frontier/round portfolios** | (a) per-leaf config portfolios for `solve`'s 100 %-cap-pinned frontier (heavy-tailed censoring is the textbook diversity case; coordinate ownership with `solve`); (b) concurrent refinement rounds at several bounds (PV shortening, priority 2). Config-rotating restarts on one core are subsumed and closed (research #20, plan20 Phase-0 PIVOT) | varies | varies | varies | **open, lowest priority** |
| 11 | **Portfolio ship shape: `examples/portfolio` driver** | Owner decision 2026-10-12 (resolves report8 §Ship-shape): option 2 — a Rust example driver (`examples/portfolio.rs`) launching the racer set (default salts 0/2/3/5, overridable) of unmodified solvers, take-first-decisive, the stop-when-proven kill policy (report8 cost finding: reclaims the grace-burned core-seconds), per-racer evals/wall/censoring accounting with the plan8 censoring discipline (budget-censored never misreported as proven draw). Fixed racer flags `--first-outcome --outcome-only`; RAM = N × TT documented in the module header; solver binary located via the cargo layout (explicit `--solver` override for tests). `--race` product flag deliberately deferred — revisit only if #6/#7 produce a scalable cooperative mechanism | the plan8 win as a durable tool; harness skeleton for #6/#7 | none (examples/tests/docs only; zero `src/` diff) | S | **closed 2026-10-12 (plan9; `report9.md`)** — shipped per plan: driver + 9 unit tests + 2 smoke tests (`make test` green), zero `src/` diff; both plan8 anchors reproduced exactly (dec10 s2 2,918,636; m20 s5 791,401,553); stop-when-proven measured (m20 race ≈ 248 core-s vs 747 with grace) |
| 12 | **Low-tax solved-facts carrier (plan11)** | The value channel survives report10's reject (m20 0.66× evals / 0.82× wall); rebuild the carrier so all three plumbing artifacts are structurally absent: key-only emission (`search::dfpn::FactExporter` observer hook, synchronous buffered writes on the search thread, proof-event sender stays `None`), threadless chunk-boundary import (the `FactImporter` hook reads sibling files directly, 1 MiB/call cap), fixed near-root admission filter (`PATH_LEN_CAP = 24`); record v2; no escalation arm | H-mech pre-registration: M1 solo wall ratio ≤ 1.03× (plan10: 1.19–1.25×), M2 pure-R0 in-race ≤ 1.06× (plan10: 1.38×); H1/H2 at plan10's 0.9×/1.1× lines; any non-adopt closes the cooperative-portfolio thread (second strike) | m20-class conversions without plan10's uniform tax | M | **closed 2026-10-13 NO-GO (plan11; `report11.md`)** — the carrier shipped and is solo-free (M1 PASS: 0.995–1.006× vs ≤ 1.03; export inert at the CLI level, drift gate clean) and the m20 value channel got stronger (H1 fired at 0.575× evals / 0.601× wall), but the pre-registered verdict is **reject**: M2 fails (stress 1.141×, m22 1.110× vs ≤ 1.06×) and H2 fails on stress (1.172×/1.158×) + dec10 (1.152×/1.088×). Diagnosis: the M2 excess is the environment's 4-way contention floor, not the carrier (fresh facts-free A0-control shows pure R0 at 1.24–1.25× its solo wall on dec13 — the ≤ 1.06 ceiling was miscalibrated for the current binary), so the surviving cost channel is TT pollution (imports perturbing winning basins: dec10 winners shifted off the 2.55 M basin in 4/5 rounds; stress cooperators perturbed off their 238–250 M basins, R0's 305 M anchor winning 4/5). Per the verdict rule the **cooperative-portfolio thread closes for good** (second strike; plan8/plan9 salt portfolio remains the family's only adopted lever)

## Non-goals

- **Reopening deterministic parallelism** (measured no-go, `lean` plan7:
  1.47–1.48× ceiling on m22/shuffle-win; the AND-node early exit is
  already harvested sequentially).
- **Cross-worker reuse of solved Win/Loss facts** (plan10 hazard class,
  `research/structural_floor.md` §2) — a parallel variant of the same
  mistake inherits the same no-go.
- **Default-on parallelism**: the sequential solver remains the default;
  the drift-gated product never depends on thread scheduling.
- Unsound speed: any design that cannot state why it never returns a
  false decisive outcome is not implementable here.

## Measurement conventions

- Primary metric: **wall time** to first decisive outcome on the
  m22/shuffle-win class (the same validation cases as `lean` plan7).
- Secondary: total `child_evals` (parallel overhead = search-work inflation
  vs the sequential baseline, the Kaneko paper's "<15% overhead" measure).
- Sequential-path gate: bit-identical stdout on the quick suite and the
  m22 first-outcome/default captures, unchanged from `lean`'s protocol.
- Parallel-mode reporting: N, per-worker work split, wall speedup vs N=1,
  and outcome agreement across repeated runs (no `wrong` ever).

## History

- **2026-10-13 — plan11 complete: low-tax solved-facts carrier → REJECT,
  backlog #12 closed, cooperative-portfolio thread CLOSED for good
  (`report11.md`)** (artifacts under `measurements/plan11/`): stage-0
  carrier rework shipped (record v2 + `PATH_LEN_CAP=24` filter,
  synchronous `FactsExporter` via the new `search::dfpn::FactExporter`
  hook, threadless chunk-boundary `FactsImporter`; threads/queue/A2 arm
  deleted; drift gate clean: quick 59/59 eval-identical, m22 default/FO +
  shuffle-win FO stdout byte-identical, TT-snapshot sha unchanged; `make
  test` green with 12 new facts tests). Stage-1 M1 PASS (0.995–1.006× vs
  ≤ 1.03) after discovering and removing an environmental confound: the
  overlay-backed repo tree's writeback blocking on exporter flushes costs
  15–23% wall on solo sub-second runs (task-clock identical,
  context-switches 1 → 886) — the same confound inflated plan10's stage-1
  solo numbers; facts dirs moved to tmpfs (documented in
  `measurements/plan11/env.json`, confounded pass kept as
  `probes11_overlayfs_confound.json`). Stage-2 canaries clean. Stage-3
  campaign: H1 fired on m20_white (0.575× evals / 0.601× wall, one
  winnerless round), dec13 improved (0.774×), rem12 clean (0.988×), but
  M2 failed (stress 1.141×, m22 1.110× vs ≤ 1.06×) and H2 failed on
  stress (1.172×) + dec10 (1.152×). Soundness/fidelity all green (zero
  conflicts, zero CRC failures, canaries clean, R0 evals bit-identical to
  the fresh solo anchors in every finished round; censoring discipline
  held incl. the winnerless m20 r4 all-censored round). Attribution
  resolution: the plan10 solo tax was plumbing + filesystem confound
  (mostly the latter); the M2 ceiling was miscalibrated for the current
  binary (facts-free contention floor alone is 1.24–1.25×); the residual
  regression channel is TT pollution. Verdict rule applied verbatim:
  reject → thread closes (second strike).
- **2026-10-13 — plan10 complete: cooperative portfolio (cross-racer
  solved facts) → REJECT/NO-GO, backlog #6 closed (`report10.md`)**
  (first `src/` diff of the initiative: `src/facts/` +
  `search::dfpn::FactImporter` + `--facts-out`/`--facts-in`; artifacts
  under `measurements/plan10/`): stage-0 drift gate clean; stage-1 paired
  export-inertness byte-exact (plan12/plan8 anchors found stale for the
  current binary — re-baselined onto fresh paired solo anchors);
  stage-2 canary clean on m22 + stress (A2 Loss-only fallback never
  triggered); stage-3 campaign 5 rounds × 6 cases interleaved A0/A1.
  Soundness clean everywhere (0 conflicts / 0 panics / 0 CRC failures in
  58 races; no Draw ever crossed a process boundary; m22 wall ratio 1.21
  — nowhere near the dfpn report10 39× collapse signature). Verdict per
  the pre-registered rule: **reject** — H2 wall failures on stress/m22/
  dec10/rem12/dec13 (1.21–1.32× `W_first` at 1.00–1.09× `evals_first`),
  diagnosed as the facts-plumbing tax (per-event path-clone emission on
  the search thread + one writer/reader thread per racer: +15–25 % wall
  even solo, +20–38 % under 4-way contention), not a search regression;
  H1 fired on m20_white (0.66× evals / 0.82× wall; winner below its own
  solo basin minimum via near-root subtree dedup) with a 1-in-5
  winnerless tail (m20 A1 r3 all-censored). Facts-channel totals: 5.45 M
  records delivered, 186 k accepted, 0 checksum failures. Follow-up
  options recorded in the report; #7 untouched.
- **2026-10-11 — plan8 complete: salt-seeded process portfolio → ADOPT,
  backlog #5 closed WON (`report8.md`)** (zero product-code changes; all
  artifacts under `measurements/plan8/`): stage-0 identity gate 5/5 exact;
  stage-1 pre-computation frozen from the committed plan12 rollout before
  any race (pointwise dominance confirmed: evals_first = predicted basin
  minimum on every raced case); stage-2 rem12 extension found rem12 NOT
  salt-invariant (6 basins, spread 1.31×, all uncensored `win`) — kept as
  the out-of-sample raced case; pilot clean; campaign 3 rounds × 6 cases
  (extension trigger never fired; max W_first spread 0.7 %). Verdict
  mechanical: **adopt** (H1 ∧ H3 ∧ H4; H2 gated dec10 clause also passed).
  Headlines: m20_white shipped 2 B-eval timeout converted to a proven win
  at 791,401,553 evals with `W_first` median 95.89 s (contention on the
  winner 1.023× — essentially nil); dec10 0.768× wall / 0.685× evals;
  rem12 0.889× wall out of sample; no regression anywhere (≤ 1.063×);
  determinism survives 4-way concurrency bit-for-bit (63/63 anchor
  reproductions); zero decisive-outcome conflicts. Cost: ~3.9× CPU
  family-wide; m20 ~747 core-s per answer (vs a shipped run that never
  answers). Ship-shape decision space (operator pattern / `examples`
  driver / product flag) recorded in `report8.md` — owner's call; #6/#7
  pre-registrations now read the confirmed draw-luck regime.

- **2026-10-11 — initiative reopened (owner decision); plan8 drafted
  (docs only, no code)**: backlog #5–#10 derived from
  `reexamination.md` §5 + its addendum (config arms closed by research
  plan13/17/18; #20 competitor closed plan20; salt = the surviving
  basin-crossing diversity source). `plan8.md` pre-registered: salt-seeded
  4-racer portfolio {0, 2, 3, 5}, take-first decisive, evaluated under the
  pinned gate v1.0 with a declared portfolio-shaped comparison adaptation;
  the evals-space result is pre-computed from the committed plan12 rollout
  (pointwise ≤ shipped run; predicted win surface = m20_white
  timeout→proof conversion + dec10 0.685×); the campaign measures only
  contention wall, W_first, out-of-sample rem12, and in-race fidelity.
  Stays closed: deterministic sibling parallelism, process-per-child
  option C, Čížek option D (non-goals unchanged).

- **2026-10-09 — re-examination note (`reexamination.md`, docs + read-only
  probes, zero product-code changes)**: plan7 portfolio flaw identified
  (`--refine-cap` inert under `--first-outcome`; ε = 0.25 measured-worst on
  both motivating cases) and a diversified ε race measured 0.63× / 0.75× /
  0.91× (shuffle-win / rem12 / m22), passing plan7's H1/H2; SPDFPN
  TT-capacity confound and coordinator-only-finish shape recorded as
  hypotheses; avoidable sequential tax and priority-1 gate mismatch noted;
  ranked ideas A–F. `report7.md` addendum appended. Measurements:
  `measurements/reexam/`. No `docs/plans/README.md` row edit (no
  open/pivot/close event).

- **2026-10-04 — plan5 complete: stage 2 (2a + 2b) → NO-GO verdict, condition-1 revert executed,
  backlog #4 closed, initiative closed (`report5.md`)**: stage 2a built the opt-in `--threads N`
  mechanism (`report5a.md`); stage 2b locked W = 10 000 (pre-campaign sweep over
  {250, 1 000, 4 000, 10 000, 20 000, 50 000}, m22 t4 — the landed 20 000 was 3× worse at
  median), ran the pre-registered interleaved campaign (5 reps × N ∈ {2, 3, 4} on m22 +
  shuffle-win, N = 1 in-session references, sequential evals 14.16 M / 249.5 M via the
  move-order suite), and produced the verdict: soundness clean (29/29 decisive `win`, zero
  panics/lock anomalies; 2 cap-hit timeout draws recorded separately), **S4 = 1.79× (m22) /
  2.68× (shuffle-win), I4 = 2.06× (m22) / 1.41× (shuffle-win) → NO-GO** (I4 > 2× fires
  regardless of speedup; m22 misses S4 ≥ 2.0× too; verdict robust to the tax-adjusted
  denominator and to the N = 3 inflation). Inflation driver identified but not chased
  (pre-registration forbids mid-campaign mechanism changes): in ~1 of 5 runs helper-stored TT
  entries steer the coordinator into a bad trajectory. **Condition 1 executed**: stage 1b + 2a
  reverted — 0-line code diff vs pre-plan4b (`e18e53e`), `make test`/fmt/clippy green, quick
  suite 59/59 `child_evals` identical, snapshot sha `eaa5f2b9…`/195 B identical; the ~5 %
  TT-concurrency tax is refunded. N = 1 drift was zero at every stage-2b checkpoint (condition 2
  held). N ∈ {8, 16} reported as clearly-labeled log-fit extrapolations only (condition 3).
  The parallelism space as scoped is now measured out end to end (plan2 option C, lean plan7
  deterministic, SPDFPN stage 2); re-open levers recorded in `report5.md`. Measurements:
  `measurements/plan5/` (sweep + campaign + drivers). `docs/plans/README.md` row updated
  (close event).

- **2026-10-04 — plan5 complete: stage 2a mechanism built, smoke sound, speed
  below GO bands (`report5a.md`)**: `--threads N` (N > 1) landed over the
  sharded TT — with one declared architecture deviation: the pre-registered
  pure worker pool could not converge on the hard class (instrumented
  diagnosis: fine-grained jobs re-verify saturated regions without advancing
  the global proof-number state), so the landed shape runs the unmodified
  sequential chunk loop on the coordinator while N−1 SPDFPN helper workers
  (W = 20 000 child evals, path-derived thresholds, virtual win/loss
  steering, zero-progress retirement, serialized events, per-worker
  repetition caches) pre-warm the shared TT. N = 1 byte-identity re-proven
  (quick suite 59/59, m22/shuffle stdout + snapshot hashes identical to
  plan4b). Smoke: outcome agreement 15/15, zero panics; 1.42×/4 threads,
  0.92×/2 threads, inflation 2.2–2.4× — below the pre-registered GO bands;
  plan5b's W sweep + interleaved campaign decide the verdict (condition-1
  revert-if-missed stands). Kill-point check: no decisive disagreement, no
  panic/torn state — the prototype stays alive. Measurements:
  `measurements/plan5/`. No `docs/plans/README.md` row edit (initiative
  neither opened, pivoted, nor closed).

- **2026-10-04 — plan4b complete: sharded-TT refactor re-landed, tax pinned, gate PASS**
  (stage 1b of backlog #4; `report4b.md`): the preserved plan4 attempt was
  applied verbatim (`git apply` + recreated `shard.rs`, header stripped);
  `cargo fmt`/`clippy`/`doc` clean, `make test` green, working-tree scope
  exactly the recorded 14 files + `shard.rs`. Drift protocol zero
  everywhere: quick suite 59/59 bit-identical, m22/shuffle-win stdout
  byte-identical, snapshot dump sha256 `eaa5f2b9…`/195 B identical to the
  plan4 record, golden test ok. Tax pinned by interleaved A/B medians:
  **m22 +5.36 % (10 pairs) / shuffle-win +4.61 % (3 pairs)** — inside the
  ≤ +7 % budget, consistent with plan4 (+6.0 %/+4.4 %; Δ ≤ 0.64 pp, no
  profiling trigger). One deviation: the revived 4-thread stress test
  flamed a latent race **in the test itself** (concurrent eviction +
  own-phase-2 refill misread as a downgrade; flaky 4/5 in isolation);
  fixed test-side only, solver code identical to the plan4 attempt.
  Measurements: `measurements/plan4b/`. Next: plan5 (stage 2a mechanism).

- **2026-10-03 (later the same day) — owner decision: A-stage reopened,
  gate re-registered (docs only, no code)**: the owner accepts the
  measured ~5 % TT-concurrency tax as the entry fee for the only
  remaining multiplicative lever, with three pre-registered conditions:
  (1) acceptance is conditional — plan5 missing its GO bands reverts the
  refactor and the tax; (2) the plan4 gate (≤ +2 %) is replaced by an
  explicit ≤ +7 % median hard-class wall budget with the byte-identity
  surface unchanged; (3) wall measurements at 2–4 threads are real (4-CPU
  container), N ≥ 8 simulated only. Speedup estimate table (2–10 threads,
  literature-anchored: SPDFPN 0.74 eff/16, Solrex ≈5.2×/4) recorded in
  the session record; expected ≈ 2.9–3.2×/4 threads, conservative band
  ~1.8× at the GO edge. Backlog #4 row updated; plan4b (re-land + pin the
  tax) and plan5 (prototype) to be written by the next session.

- **2026-10-03** — **plan4 complete: inert TT-concurrency refactor → NO-GO at the
  pre-registered kill point; backlog #4 closed** (code reverted; docs +
  measurements committed): the sharded, interior-mut, `Send + Sync`
  `TranspositionTable` was fully implemented and behaviorally inert
  (quick-suite `child_evals` bit-identical per case, m22/shuffle-win
  stdout byte-identical, snapshot dump byte-identical, `make test`
  green, new concurrency-invariant tests included), but the wall gate
  failed reproducibly: **+6.0 %** m22 / **+4.4 %** shuffle-win median
  first-outcome vs the ≤ +2 % threshold. Profiled per the plan's risk
  protocol (`perf record`): the RMW pair is only 0.7 % — the cost is the
  lock discipline at child-eval granularity (~15–20 probes/node,
  ~15 M probes on m22; 48 B copies, non-forwardable loads, clobber
  re-loads). `#[inline]`, unchecked indexing (probe inlined, no gain)
  and `-Ctarget-cpu=native` (LSE) were each wall-neutral; a seqlock
  redesign (lock-free readers, atomic entry fields) was scoped and
  rejected: still projects ≈ +2–3 % and adds version-retry storms under
  plan5's multi-writer contention — worse for the target workload. The
  refactor was reverted to the byte-identical sequential solver (clean
  `git diff`, gate re-run green); the full attempt is preserved in
  `measurements/plan4/sharded_tt_attempt.patch` + `shard_rs_attempt.rs`.
  Report: `report4.md` (includes the reopen triggers: a consumer-facing
  GO re-weighing of a ~5 % sequential tax against the parallel band, or
  a mechanism redesign with a re-registered gate — owner's call).
  `docs/plans/README.md` row updated (close event).

- **2026-10-02** — **initiative reopened; plan3 complete: SPDFPN 2014
  mined, option-A GO input** (docs only, no code): premise change
  recorded (nondeterministic parallel runs accepted); paper located via
  author copy after the bibliography's arXiv pointer proved wrong
  (arXiv:1503.07698 resolves to an unrelated XENON1T paper — entry
  corrected, flipped **Open → Mined**); extraction vendored under
  `docs/theory/spdfpn-2014/`; `research_spdfpn.md` written (mechanism,
  measurements, soundness mapping, bill of materials, staged plan
  skeleton); backlog #4 opened. `docs/plans/README.md` row updated
  (reopen event).

- **2026-09-21** — **plan2 complete: architecture selection → measured
  NO-GO** (no `src/`/`examples/` change; spike drove the release binary
  as a black box): `design_space.md` written (A–D × constraints matrix,
  ship-shape space, measured cells, decision); sizing spike executed in
  the pre-registered order — perspective-mapping validation vs
  `find_winning_child` (exact agreement, dec08/dec03), sequential
  baselines (m22 2.63 s/858 k nodes; shuffle-win 47.8 s/13.9 M),
  per-child tables (43+41 children, 120 s caps), races at N=2/3/4 ×
  5 reps (0.29×/0.28×/0.27×; 11.6–15.5× inflation; zero decisive
  disagreements), shuffle-win probe (race cannot reproduce the root
  proof), RAM (≈230 MB/worker; N×TT restatement vs the enforced 8 GiB
  cgroup), N=8/16 schedule simulations (0.30×, N- and order-insensitive;
  labeled simulated). Bibliography: Pawlewicz & Hayward 2014 added as
  **Open**. Backlogs #2/#3 closed; `docs/plans/README.md` row updated
  (close event). Finding that closes the door: root-level cross-child
  transposition sharing + best-first interleaving are worth ≈10× on
  m22 and are structurally inaccessible to process-per-child workers.

- **2026-09-21** — **plan1 reading round complete** (docs only, no code):
  backlog #1 mined — `research_cizek2025.md`, `research_jlpns.md`,
  `research_solrex.md` written in this directory (PDFs vendored under
  `docs/theory/`);
  cross-paper comparison paragraph added to the option-space section;
  backlog #2 enriched with the synthesis pointers and spike order.
  Bibliography: Čížek 2025, JLPNS, Young 2016 flipped **Open → Mined**.
- **2026-09-21** — **initiative opened; housekeeping handover** (docs
  only, no code): `conversion` #4 and `lean` #2 (parked dormant after the
  plan7 spike) consolidated here. Bibliography pointers for the three
  open parallelism entries (JLPNS, Young 2016, Čížek 2025) re-targeted to
  backlog #1. Reopen-trigger renegotiation recorded in Status. Kaneko
  mining stays where it was mined (`dfpn/research_parallel.md`); the PDF
  remains at `docs/theory/pdfpn-2010/pdfpn-2010.pdf`.

Per repo convention, every plan ends with the task of writing its
`report<N>.md` in this directory.
