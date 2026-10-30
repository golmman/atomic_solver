# Solver features and techniques

This file is the consolidated inventory of search-efficiency techniques in
`atomic_solver`: what the solver currently uses (default-on unless marked
*opt-in*), and what was researched, measured, and rejected. It is a map to
the evidence, not a new measurement record — every impact figure is taken
from (or derived from) the cited report, and older plans/reports may not
reflect the current code.

## How to read the impact column

`impact` is a rough wall-time factor for applying the feature to the cited
corpus, relative to the same solver without it:

- `0.5` means the feature roughly halves wall time (about 2× faster);
- `0.9` means about 10% less wall time;
- `1.0` means no measurable effect;
- `> 1.0` means the feature made the measured workload slower.

The figures are **not additive** across rows: the techniques were measured
against different baselines, on different hosts (aarch64 reference VM vs.
x86_64 container; see `plans/lean/initiative.md`), and some are
class-dependent (hard/deep cases vs. short solves). Where a feature helps one
class and hurts another, the table gives the value for the case that drove
the verdict, and the name/references record the rest. A few rows are estimates
without a clean local A/B (notably the foundational search rules, where the
baseline "without the feature" is not buildable); the references say so.
Deterministic `child_evals` is the preferred metric in this repo; wall time on
this container is only stable to about 1–3%
(see `performance/performance_report1.md` §2).

## Currently used features

| id | name | impact | references |
| --- | --- | --- | --- |
| F1 | 1+ε threshold rule (DF-PN+, estimate) | 0.8 | `theory/epsilon-trick-2007/` (Pawlewicz & Lew 2007), `plans/dfpn/research_epsilon.md`, `plans/research/structural_floor.md` §2 (ε surface: no constant beats the default without control regressions; ε=0.5 is 0.63 on stress but regresses m22/dec13), `src/search/dfpn/core.rs` |
| F2 | Iterative work chunking / restart schedule | 1.0 | `plans/ultimattt/plan5.md`, `plans/ultimattt/report5.md` (origin), `plans/conversion/plan12.md`, `plans/conversion/report12.md` (restart device is load-bearing for tails: dec31 278×, m23_white unprovable within 20 s without a boundary; ~21% tax on stress first-outcome at 256 MB), `src/search/dfpn/restart_schedule.rs` |
| F3 | Move order heuristics (static scorer: pawn storm, rook centralization, atomic SEE, near-commoner) | 0.5 | `plans/move_order/report2.md`, `plans/move_order/report3.md`, `plans/move_order/report4.md`, `plans/lean/report8.md`, `plans/lean/initiative.md` Motivation (ordering is at its measured floor: a perfect per-node OR ordering would only save a further 0.509× work-weighted, with 90.6% of OR work already on the decisive child; no no-ordering ablation exists, so the factor is an estimate), `src/search/ordering.rs` |
| F4 | History/killer tables + TT best-move ordering (estimate) | 0.9 | `plans/lean/report10.md` (re-tuning closed no-go: the shipped constants are the local optimum), `plans/ultimattt/report3.md` (best-child hint), `src/search/dfpn/history.rs` |
| F5 | TT bucket slots and replacement policy (4-slot bucket, work/solved/generation priority) | 0.74 | `plans/lean/report14.md` (4 slots at identical entry count/RAM; stress censored-win, wall −25.7%), `plans/lean/report13.md` (retention probe), `plans/ultimattt/report3.md`, `src/search/tt/table.rs` |
| F6 | TT capacity default (64 → 128 MB, now 256 MB label) | 0.5 | `plans/lean/report4.md` (m22 first-outcome −49% wall / −62% evals; default mode −74%), `plans/lean/research_tt_capacity.md`, `plans/lean/report15.md` (label honesty; default table unchanged) |
| F7 | TT child-bucket prefetch pre-pass | 0.75 | `plans/lean/report11.md` (−23.8% m22 / −26.4% shuffle-win on x86_64; removing it makes m22 ~1.9× slower per `performance/performance_report1.md` §5), `src/search/dfpn/children.rs`, `src/search/tt/table.rs` |
| F8 | Non-mutating `hash_after` keys for the prefetch pre-pass | 0.87 | `plans/lean/report12.md` (−12.5% m22 default, −12.9% m22 FO, −12.6% shuffle-win, −13.9% quick suite; residual pre-pass ≈5.2% of wall), `plans/movegen/plan_hash_after.md` |
| F9 | TT pre-probe before make/unmake | 0.95 | `plans/performance/report3.md` (dec10 0.946 wall, other deep cells 0.964–0.982; 7.9–38.1% of child evals skipped), `performance/performance_report1.md` §7.3 |
| F10 | Cached `ChildInfo` across DF-PN loop iterations | 0.4 | `plans/speed/report11.md` (3–7× fewer per-node child evals, 2–3.5× faster on loop-heavy positions), `src/search/dfpn/core.rs` |
| F11 | Per-search repetition cache (path-context-keyed draws) | 0.71 | `plans/dfpn/report9.md` (stress first-outcome 351M → 249M child evals, −29%; default mode −27%), `plans/dfpn/research_repetition_cache.md`, `src/search/dfpn/repetition_cache.rs` |
| F12 | Incremental repetition-context hash + path Bloom filter | 0.96 | `plans/performance/report4.md` (dec10 0.9630 wall / 0.9614 CPU; deep cells 0.987–0.996; zero-slot negatives on 89.9–99.1% of lookups), `src/search/dfpn/mod.rs` |
| F13 | TT generation counter instead of `clear()` | 0.95 | `plans/speed/report10.md` (small solves 2× faster, large searches ~1.0; keeps TT across bootstrap/refinement generations), `src/search/tt/table.rs` |
| F14 | Lazy zero-initialised TT allocation (`alloc_zeroed`; short-solve latency, +3% on deep runs) | 0.05 | `plans/performance/report2.md` (mate-in-1 104.6 → 1.2 ms / 229 → 16 MB; deep cells +2.7–3.6% wall), `src/search/tt/alloc.rs` |
| F15 | Detector-gated 3-man preflight (region-closure fixpoint; draw/hard class, >1 on easy wins) | 0.1 | `plans/dfpn/report13.md` (KQ ladder root certified at 5.87M evals / 0.76 s; KR/KN/KB draws 0.28–0.79 s vs. DF-PN timeout), `src/search/preflight/` |
| F16 | Cheap bounded DF-PN probe before preflight (easy 3-man wins; +2.5% elsewhere) | 0.02 | `plans/performance/report1.md` (easy KQ wins 0.75–0.80 s → 0.010–0.011 s, 5.83M → 2–586 evals; draws/hard wins +≤0.087% evals, ≤+2.5% wall), `src/search/dfpn/cheap_probe.rs` |
| F17 | First-outcome mode (`--first-outcome`) | 0.8 | `performance/performance_report1.md` §3.3 (refinement adds ~25% evals even when it cannot shorten the line), `src/search/dfpn/run.rs` |
| F18 | Deterministic refinement cap (`--refine-cap`, default 0.25) | 0.8 | `plans/lean/report1.md` (per-round cap bounds futile refinement; `--refine-cap 0` recovers the uncapped tail at large cost), `plans/dfpn/initiative.md` #3 (cap-0 equivalence and measured poor PV value) |
| F19 | Hot-path micro bundle (scratch-slot make/unmake, clock sampling, TT non-terminal skip) | 0.93 | `plans/lean/report6.md` (m22 first-outcome −7.1%, shuffle-win −5.9%, bit-identical trajectories), `plans/lean/initiative.md` #12a/#13/#14 |
| F20 | Cheaper static scoring (const Chebyshev table, ray pre-filters, bitboard aSEE) | 0.97 | `plans/lean/report8.md` (−3.0% m22, −2.2% shuffle-win, −3.0% quick suite; bit-identical scores) |
| F21 | OR-node move-class starting proof numbers (`--move-class-pn`, opt-in) | 0.13 | `plans/provari/report2.md` (8.74× fewer evals / 7.58× less wall on the core FEN; 3.41×/3.18× on the shared win; all decisive proofs validated), `src/search/dfpn/move_class.rs` |
| F22 | Quiet-move grouping (`--quiet-grouping`, opt-in) | 0.5 | `plans/provari/report3.md` (2.60×/1.97× core, 2.27×/1.85× shared win; 1.80×/1.47× over move-class alone) |
| F23 | Evaluation-scaled proof numbers, `--eval-pn or` (opt-in) | 0.6 | `plans/provari/report5.md` (core 0.618× evals, shared 0.580×, geomean 0.599×; the `both` arm was rejected at 1.483× on the shared win), `src/search/dfpn/eval_guide.rs` |
| F24 | Salt-seeded process portfolio (`examples/portfolio`) | 0.77 | `plans/parallel/report8.md` (dec10 0.768× wall / 0.685× evals; m20_white converted from budget-censored to proven at 791M evals; ~3.9× CPU family-wide), `examples/portfolio` |
| F25 | Release profile (fat LTO, 1 codegen unit, `panic=abort`) | 0.98 | `plans/speed/plan1.md`, `plans/speed/report1.md` (~2%, at the edge of noise), `Cargo.toml` |
| F26 | Path stack + linear scan (instead of `HashSet`) | 0.99 | `plans/speed/report6.md` (~1% on the longer decisive case; removes per-move hashing/allocations), `src/search/dfpn/mod.rs` |
| F27 | Incremental board hash + compile-time rule50 Zobrist keys | 1.0 | `plans/speed/report9.md` (removes the per-move rule50 hash rebuild; the wall effect was within noise), `src/zobrist.rs` |

## Researched and measured but rejected

| id | name | impact | references |
| --- | --- | --- | --- |
| R1 | GHI twin table + Kawano simulation (unsound) | 1.0 | `plans/dfpn/report5.md` (implemented) and `plans/dfpn/report7.md` (removed: cyclic-rook position returned a false win), `plans/dfpn/research_ghi.md`, `plans/research/structural_floor.md` §3 |
| R2 | Clock-budget-aware solved-entry reuse (m22 control; 0.62 on stress) | 3.9 | `plans/dfpn/report10.md` (m22 3.1 s win → 120 s timeout; 15/59 quick cases regress up to +185%) and `plans/dfpn/report11.md` (seven-arm decoupling matrix: 84.8% of the adoption mass is simultaneously the stress win and the m22 destabilizer; closed permanently) |
| R3 | Bounded cross-path verification (GHI-journal "Option A"; ≤8.8% win, not worth its re-proof cost) | 0.91 | `plans/dfpn/research_ghi_journal.md`, `plans/dfpn/initiative.md` #4, `plans/conversion/report1.md` (plan9 already intercepts ~97% of the re-proofs) |
| R4 | Context-keyed threshold-cut reuse (GHI-safe side table; m20_white control; 0.656 stress) | 3.0 | `plans/dfpn/report14.md` (m20_white 3.02× regression, `dec44` censored loss, m22 10.2× at 128 MB; spike reverted) |
| R5 | Deep df-pn / DeepPN depth-scaled leaf values (seesaw reducers) | 1.0 | `plans/research/report9.md`, `plans/research/research_deep_dfpn.md`, `theory/deep-dfpn-2017/`, `theory/deep-pns-2015/` (path-dependent leaf values poison the path-independent TT; best-tuned 1+ε matches or beats Deep df-pn in 4/8 of the paper's own positions), `plans/research/structural_floor.md` §6 |
| R6 | Alternative PNS algorithms (PDS, PN², PDS-PN, DFPN-PN; published df-pn margin 2.6–4.5×) | 3.0 | `plans/research/report6.md`, `plans/research/research_alternative_algorithms.md`, `plans/research/structural_floor.md` §1 (PN²/PDS-PN frontier is RAM-contract-fatal) |
| R7 | Neural-network / learned move ordering and node priors | 1.0 | `plans/lean/initiative.md` Motivation (archived `nn` branch: oracle floor 0.509× work-weighted, 90.6% of OR work already on the decisive child), `plans/research/initiative.md` #6/#13, `plans/research/structural_floor.md` §9 (no evaluator in a pure solver) |
| R8 | Pattern databases / mating-net recognizers | 1.0 | `plans/research/initiative.md` #7, `plans/research/structural_floor.md` §8–§9 (zero harvestable ≤5-men pawnless frames: 0.000% of 50.9M decisive-suite child evals; preflight covers the only eligible roots) |
| R9 | Expected Work Search / Multiple-Outcome PNS paradigms | 1.0 | `plans/conversion/research_ews.md`, `plans/conversion/research_mopns.md`, `plans/conversion/report3.md` (EWS needs a win-rate/proof-size evaluator the solver does not have; MOPNS adds no missing draw machinery), `plans/research/structural_floor.md` §7 |
| R10 | Threshold-cut-frame pricing / partial-sum sweep short-circuit (stress 3.28× evals, m22 2.16×) | 2.5 | `plans/conversion/report6.md` (diagnostic GO) and `plans/conversion/report7.md` (net loss with outcome regressions: stress FO win → 120 s draw), `plans/research/structural_floor.md` §7 |
| R11 | ε retuning and dynamic ε scheduling (0.375/0.5, depth/clock schedules; control veto, 0.91 stress at ε=0.375) | 1.2 | `plans/research/report4.md` (ε=0.5: stress −37.3% but m22 +12.7%, dec13 +52.4%; schedules +28% … +351%), `plans/conversion/report8.md` (default 0.375: quick 57/59 and thorough 34/66 regressions), `plans/research/structural_floor.md` §2 |
| R12 | Oracle-path ε (exogenous PV-conditioned threshold padding; best arm, below the −10% gate) | 0.98 | `plans/conversion/report10.md` (best pad arm +1.8% on stress; the equal-value global control G050 is −37.3%, so the exotic conditioning adds nothing) |
| R13 | History/killer constant re-tuning (dec14 tail; dec05 5.19×) | 1.6 | `plans/lean/report10.md` (23-arm sweep: every aggregate-winning arm flips a solving case to timeout or regresses a tail), `plans/research/report18.md` (gate-grade replication), `plans/lean/initiative.md` #10 |
| R14 | TT eviction V2 / solved-entry pinning (stress +38.2%, m22 +23.7%) | 1.3 | `plans/research/report19.md`, `plans/lean/report7.md` (arm V1 never terminates the stress case), `plans/research/structural_floor.md` §5 |
| R15 | Salted TT-keeping restarts (reseed bucket index, keep entries; mechanism fails before rollout) | 1.0 | `plans/research/report20.md` (re-placing a ~95%-full 2-slot TT drops 15–23% of live entries per reseed; Phase-0 kill gate fired on every stress probe) |
| R16 | In-context child results (keep a just-proven repetition Draw in the parent frame; dec02 veto, 0.54 on dec10) | 1.12 | `plans/research/report21.md` (dec10 median 0.54× and H1 confirmed, but deterministic dec02 1.117× on all salts vetoes; m22 salt-5 6.50× tail) |
| R17 | Leaf-initialization family (mobility-shaped `(pn, dn)` for fresh children; hard-class veto, 0.16–0.24 on stress/dec13) | 11.0 | `plans/research/report15.md`, `plans/research/report16.md` (m22 censored at 1B evals on all salts; dec01 censored-loss 11–55×; no product change) |
| R18 | Deterministic sibling parallelism (lazy-SMP / OR-AND split; structural ceiling, below the 1.5× bar) | 0.68 | `plans/lean/report7.md` (OR/AND split gives a 1.47–1.48× ideal ceiling with zero overhead; AND early-exit is already harvested sequentially), `plans/lean/initiative.md` #2 |
| R19 | SPDFPN shared-TT parallel helpers (shuffle-win; 2.22× work inflation) | 1.65 | `plans/parallel/report6.md` (8-rep medians: S4 = 1.01×/1.20×/1.65×, I4 = 2.22–3.54×; plan5b's 2.68× was a 5-rep artifact of a bimodal tail), `theory/spdfpn-2014/` |
| R20 | Process portfolio over config diversity (`--refine-cap`/`--epsilon` racers; 0.986×, 4.4× CPU) | 1.0 | `plans/parallel/report7.md` (solver is deterministic per (FEN, config); no sequential bimodality to exploit; 0.986× median on shuffle-win vs. the ≤0.75× gate), `plans/parallel/reexamination.md` |
| R21 | Cooperative portfolio / cross-racer solved-fact sharing (wall; 0.66× evals on m20) | 1.25 | `plans/parallel/report10.md` (facts-plumbing wall tax 1.21–1.32×; H2 fail) and `plans/parallel/report11.md` (low-tax carrier: M1 solo tax gone, but stress 1.141× / m22 1.110× and TT-pollution channel; thread closed for good) |
| R22 | Čížek-style massive / job-level parallel PNS (root-child process race; 15.5× work inflation) | 3.7 | `plans/parallel/research_cizek2025.md`, `plans/parallel/research_jlpns.md`, `plans/parallel/design_space.md` (option D has no safe shared payload; option A remains gated), `plans/parallel/report2.md` (0.27× wall / 15.5× inflation; published near-linear scaling needs large, independent, persistent jobs), `theory/pns-pdfpn-2025/`, `theory/ppn2-2011/` |
| R23 | 3-man endgame tablebases for faster solves (no invocation sites) | 1.0 | `plans/egtb/report1.md` (non-terminal ≤4-men share of child evals 0.000% on the decisive suite; generator retained only as a correctness oracle), `plans/egtb/initiative.md` #1 |
| R24 | `target-cpu=native` | 1.0 | `performance/performance_report1.md` §7.5 (<1%, within noise), `plans/speed/report1.md` |
| R25 | Reusing the terminal-check `StateInfo` scratch slot | 1.0 | `performance/performance_report1.md` §7.4 (no measurable change; superseded by the shipped scratch-slot make/unmake work in F19) |
| R26 | Unstable move sort (not pursued) | 0.99 | `performance/performance_report1.md` §7.6, `plans/performance/initiative.md` #7, `plans/lean/report10.md` (tie order is search-visible; full gate required) |
| R27 | Huge pages / `MADV_HUGEPAGE` | 1.0 | `plans/lean/measurements/plan11_phase0/README.md`, `plans/performance/report2.md` §Findings 5 (no effect on this container; with lazy allocation, THP is the configuration most likely to hurt) |
| R28 | Static atomic tactical closure (`--static-tactics`; core FEN 1.18–1.24× worse, 1.49× better on the shared win) | 1.2 | `plans/provari/report4.md` (sound, 8/8 proofs validated, coverage unchanged, but not additive with the adopted provari stack; flag stays default-off) |
| R29 | Engine hybrid (Fairy-Stockfish candidate lines: ordering seed / PPV verification; control veto, verification burned >1B evals) | 1.3 | `plans/conversion/report2.md` (2b verification NO-GO), `plans/conversion/report9.md` (2a ordering seed: guided stress −15% best case but dec13 +31% and m22 +58%), `plans/conversion/initiative.md` #2 |
| R30 | Clock-pressure ordering signal (rule50-aware bonuses; trigger surface 0.002% of child evals) | 1.0 | `plans/conversion/report4.md` Phase 0 (high-clock AND frames are 0.011% of nodes and 0.002% of child evals on stress FO — 2–3 orders below the ≥10% bar; both halves closed) |
| R31 | Restart-schedule retuning (chunk base/factor/cap; no dominating arm, 1.21× stress tax at 256 MB vs. tail rescue) | 1.0 | `plans/conversion/report12.md` (S0–S5 grid: no arm improves stress FO ≥10% at both TT sizes without a loss or tt-128 timeout; `500_000` geometric ×2 kept) |
| R32 | Focused DF-PN child limits / dynamic widening (FDFPN, Yoshizoe; structurally inverted vs. the churn mass) | 1.0 | `plans/research/report5.md`, `plans/research/research_child_termination.md`, `theory/` (Henderson 2010 cited in `bibliography.md`), `plans/research/structural_floor.md` §7 |
| R33 | Proven-Win child re-evaluation / "reuse solved facts in the live search" beyond the TT (subsumed or measured out) | 1.0 | `plans/parallel/report10.md` (cross-worker), `plans/research/report21.md` (in-context), `plans/research/structural_floor.md` §3/§5 |

## Notes and open surface

- Default-off, measured-stack levers (F21–F23) are implemented and validated
  but not product defaults; `provari` item 7 owns the default-stack decision
  (`plans/provari/initiative.md`).
- Items deliberately left out of the rejected table because they are open,
  not rejected: `lean` #7 (lazy/staged child evaluation), `provari` #2
  (win-only front-end), #6 (rule50-free key + proof length) and #8 (sibling
  proof replay), `proof` #5/#7 (TT checkpoint, deep-proof capacity), `egtb`
  #2–#5 (4-man layer), `research` #22 (fringe).
- The resource contracts that bound the search space (RAM = TT only,
  path-independent TT, deterministic child-eval budget) are stated in
  `AGENTS.md` and `plans/research/structural_floor.md`; a feature that
  violates one is not a candidate regardless of wall time.
