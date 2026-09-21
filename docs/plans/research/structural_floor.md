# The structural floor — the `research` initiative's consolidated no-go record

**Date:** 2026-09-21. **Status:** closing deliverable #16 of the
[`research`](initiative.md) initiative (documented by `plan10.md` /
`report10.md`).

**What this document is:** the single, authoritative statement of what the
solver is locked into and why. Every locked-in commitment below is followed
immediately by its measured or mined evidence; every claim traces to an
existing `reportN.md` or `research_*.md` in this repository — the
verification audit is [`measurements/plan10/claims.md`](measurements/plan10/claims.md).

**What it is not:** a specification (the evidence chain references
repo-internal process records, which `docs/spec/` forbids); and not a
promise that no lever exists anywhere. It records where the *measured and
mined* record stands as of the date above; each section's evidence also
records what was never run, so the reader can see the floor's edges.

**Reading conventions.** The solver's hard-case baselines are, per the
post-plan9 conventions in [`initiative.md`](initiative.md): the *stress
case* (`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`, =
`m21_white`) at **249,480,478** first-outcome child evals, and `m22_white`
at **14,156,269**, both on a 128 MB TT (reference host). "Class (b)" is the
initiative's classification for a *sound, unmeasured, contract-compliant
mechanism* — the only class that would justify new work; the full classes
are (a) already implemented, (b) sound-and-unmeasured, (c) contract-breaking,
(d) evidence-absent-or-against. "GO/NO-GO" are the initiative's pre-registered
gate decisions (GO = a measured ≥10% first-outcome win on the stress case
with controls preserved; NO-GO = the lever closes). "Gate object" = the
stress case; "controls" = m22_white, dec13, dec10.

---

## 1. The DF-PN+ commitment and its measured cost

The solver is committed to **DF-PN+ (depth-first proof-number search with
the 1+ε second-child threshold rule)**. The commitment is a measured local
optimum by published evidence, not a default inherited without comparison:

- The only two published direct df-pn-vs-PDS comparisons ever run —
  Pawlewicz & Lew 2007's Atari Go 6×6 TT-size sweep and 286 hard LOA
  positions — favor df-pn by **2.6–4.5× in solving time**, with the largest
  margins exactly in the "search tree ≫ TT" regime that defines the stress
  class. Evidence: [`report6.md`](report6.md);
  [`research_alternative_algorithms.md`](research_alternative_algorithms.md)
  §2.3 and §3 (rows 1–3).
- **PN²** is contract-fatal here: its level-2 best-first frontier is
  non-TT memory by construction, and the published record independently
  shows PN²'s solving performance collapsing under memory restriction.
  Evidence: [`report6.md`](report6.md);
  [`research_alternative_algorithms.md`](research_alternative_algorithms.md)
  §2.2, §5, §6.
- The two-level hybrids (**PDS-PN**, **DFPN-PN**) carry the same level-2
  frontier plus a never-run comparison: the mined chapter states explicitly
  that "there has not been a direct comparison between df-pn with PDS-PN",
  and the ICGA-2012 survey concedes a comprehensive variant study "is
  sorely missing". The gap is recorded, not papered over. Evidence:
  [`report6.md`](report6.md);
  [`research_alternative_algorithms.md`](research_alternative_algorithms.md)
  §3 rows 8–9.
- Published **PDS ignores GHI outright** ("in the current PDS algorithm we
  ignore this problem"), and no repetition-dominated domain was ever
  head-to-headed. Evidence:
  [`research_alternative_algorithms.md`](research_alternative_algorithms.md)
  §2.1, §2.4.
- PDS's structural deltas against the implemented DF-PN+ reduce to: the +1
  threshold schedule (the ε→0 corner of the already-implemented 1+ε rule),
  delayed evaluation (its published 7–8× node-generation overhead), and
  TT-stored per-node thresholds (a constant-factor layout change). Evidence:
  [`research_alternative_algorithms.md`](research_alternative_algorithms.md)
  §4, §6.
- Two-level TT replacement schemes are evidence-against (the shogi-community
  consensus), corroborating the single-table design in `src/search/tt/`.
  Evidence: [`research_alternative_algorithms.md`](research_alternative_algorithms.md)
  §6 (final row).

**The floor:** every whole-algorithm variant mined (PDS, PN², PDS-PN,
DFPN-PN) lands in class (a), (c), or (d) — none in (b). Every *measured*
row in the 9-row head-to-head table favors the df-pn family. The honest
caveat is part of the floor: the evidence base is thin in exactly this
solver's regime (no repetition-dominated comparison, no df-pn-vs-PDS-PN
run) — thin, but uniformly one-sided.

## 2. The 1+ε threshold mechanism and the four ε closure legs

The search-order dial that *is* implemented is the 1+ε second-child
threshold rule (`epsilon_ceil`, `src/search/dfpn/core.rs`; the published
Pawlewicz & Lew mechanism). Its tuning surface — ε — was closed by four
independent, measured legs:

1. **No global constant Pareto-dominates.** ε=0.5 wins the stress case
   −37.3% but regresses the controls (m22 +12.7%, dec13 +52.4%); the only
   constant with no control regression, ε=0.375, wins stress −8.7% — under
   the 10% GO bar (handed to `conversion` as a sized note, not adopted).
   Evidence: [`report4.md`](report4.md) Phase 0.
2. **No depth/clock schedule beats the constant.** Four schedule arms
   (path depth, remaining depth, rule50 clock, linear ramp): the two arms
   that never fire are bit-identical to baseline; the two that fire regress
   (near-root coarsening: +28.4% stress, m22 timeout; linear: +351% m22).
   Evidence: [`report4.md`](report4.md) Phase 1.
3. **No regional structure.** The threshold response of the churn mass is
   *trajectory chaos* — per-position, non-monotone, non-additive across
   regions (m22 tolerates global ε=0.5 but times out when the same ε is
   applied only near the root). Evidence: [`report4.md`](report4.md)
   hypothesis verdicts.
4. **No exploitable node-local signal.** Conditioning ε on features read at
   the threshold-computation site (side × depth × static top-2 margin ×
   clamp state): no single feature separates rescued-from-dead cuts on any
   bucket covering ≥5% of cut-frame eval mass (best 1.94× < the 2× bar);
   composite buckets separate (2.19–2.73×) but the separation is
   trajectory-relative — it collapses at ε=0.125 — and all three
   conditioned-ε arms fail catastrophically (pad-more: +300% stress;
   pad-less: timeout at 2.71 B evals). Evidence: [`report8.md`](report8.md).

**The floor:** any future ε revisit needs a mechanism outside this entire
class — not a constant, not a schedule, not regional, not node-local
threshold conditioning ([`report8.md`](report8.md), "four-legged" record).

**The seesaw framing this forces** (from [`report9.md`](report9.md)):
search-order guidance in this solver is confined to **position-only
signals** (static scorer, history, killers, TT bounds) and **path-only
thresholds** (1+ε). Every depth-keyed *valuation* mechanism in the mined
literature is either path-dependent (Deep df-pn, §6 below) or best-first
(DeepPN, PN², §1 and §4 below) — both excluded by the TT and RAM contracts.

## 3. GHI and the path-independent TT

Repetition handling is locked into the **GHI first-player-loss shortcut**:
the first repetition on the current path is scored as a loss for the player
who first repeats, and **repetition-dependent results are never cached**.
The transposition table (`src/search/tt/`) stores path-independent base
entries only, and the Zobrist key (`src/zobrist.rs`) includes the halfmove
clock. Evidence: [`AGENTS.md`](../../../AGENTS.md) (architecture section);
`src/search/tt/` module docs.

**Why this excludes an entire mechanism family** — the plan9 crux: any
leaf-value scheme keyed to path depth (Deep df-pn's `D_dfpn(depth) =
E^(D−depth)`) makes the unsolved bounds stored at frame exit —
`(pn.max(1), dn.max(1))` in `src/search/dfpn/core.rs` — **path-relative**:
the same position reached by different lines would store different `(pn,
dn)` in `TtEntry`, and `evaluate_child`'s reuse guard
(`src/search/dfpn/children.rs`) would then import foreign-path valuations,
voiding Theorem 1's own consistency premise. A depth source *does* exist at
the leaf sites (`path_stack.len()` is already read there) — the failure is
not "no depth proxy" but "any path-derived proxy poisons the stored
bounds". Evidence: [`report9.md`](report9.md);
[`research_deep_dfpn.md`](research_deep_dfpn.md) §5.1;
[`measurements/plan9/code-sites.md`](measurements/plan9/code-sites.md)
(quoted code, read this session per plan9).

Every path-independent workaround is measured- or evidence-fatal: never
storing deep-derived bounds discards the unsolved-bounds reuse that is the
search's working set (direct analog: plan7 arm V1 — stress timeout, m22
+3400%); path-depth-tagged entries need a TT layout change and collapse
transposition reuse in the 14 M–250 M-eval regime; position-only proxies
(halfmove clock, material) mutate the mechanism and point the wrong way.
Evidence: [`report9.md`](report9.md); [`report7.md`](report7.md) (V1 arm).

**The floor:** the TT is a *position-keyed, path-independent bound store*.
Any mechanism whose value depends on how the position was reached can be
computed but never cached — and in this regime uncached means unaffordable.

## 4. RAM = TT only and the best-first exclusion

The search CLI's resource contract is **RAM = TT only**
(`src/main.rs` module header): all search state beyond the stack lives in
the fixed-size transposition table. The best-first family — plain PN, PN²,
DeepPN, and the level-2 layer of PDS-PN/DFPN-PN — holds an open frontier
*outside* the TT by construction, which is contract-fatal (classification
(c) in plan6), independent of the published memory-collapse evidence.
Evidence: [`report6.md`](report6.md);
[`research_alternative_algorithms.md`](research_alternative_algorithms.md)
§5 (contract check).

**The floor is stated precisely so it is not overclaimed.** Two bounded
exceptions exist, both documented in
[`AGENTS.md`](../../../AGENTS.md):

1. the pre-flight **region closure** — `REGION_BUDGET`-bounded memory on
   ≤3-men, pawnless, no-castling roots only (`src/search/preflight/`);
2. the reconstruct-side **memory limit** — `Search::set_memory_limited` /
   `ExitReason::MemoryLimit` are a contract of `src/reconstruct/walker.rs`
   (the offline proof-tree builder), not of the search CLI.

Throwaway POC instrumentation (e.g. plan8's 2^24-slot event table) also
exceeded the contract transiently, but only inside reverted spikes —
never shipped. Evidence: [`report8.md`](report8.md) ("Problems
encountered"); [`initiative.md`](initiative.md) working agreement #2.

**The floor:** within the search CLI, a best-first algorithm cannot be
adopted without breaking the resource contract first — and the published
record (§1) gives it no compensating win.

## 5. Ordering and TT-eviction local optima

Two neighboring surfaces were measured shut by POCs inside this initiative
and its neighbors:

- **Move ordering is at its floor.** At refuted AND frames the refuter is
  already at final-sorted **rank 0 in 100%** of frames on all measured
  cases (median rank 0); it is statically detected (extinction/mate
  terminal), found in the initial sweep, and pre-refuter eval mass is
  **0.00–0.02%** of child evals. On the OR side the winning child is at
  rank 0–1 in ~97% of OR-Win frames. The remaining 99.7–99.9% of AND own
  evals sit in *threshold-cut* frames that never reach a refutation exit —
  no ordering signal can touch them. Evidence:
  [`../lean/report9.md`](../lean/report9.md);
  [`../lean/initiative.md`](../lean/initiative.md) #5 (closed).
- **TT eviction policy is a measured local optimum.** Priority replacement
  `(live, solved, work, generation)` already existed (the "uniform
  replacement" premise was stale); harmful-class churn (new-unsolved
  evicting live-solved) is **0–0.146% of stores** at the 128 MB default and
  ≤1.21% of probes on evicted keys. Both pre-registered replacement arms
  fail: solved-slot immunity (V1) drops 15 M unsolved stores and never
  terminates the stress case (unsolved bounds *are* the working set; m22
  +3400%); steeper work priority (V2) regresses stress +38.2% / m22
  +23.7%. Any further eviction-policy work needs a TT layout change, which
  the RAM = TT only contract fixes out of scope. Evidence:
  [`report7.md`](report7.md).

**The floor:** ordering levers and eviction levers are closed as measured
local optima within the current layout; reopening either requires a layout
or mechanism change, not a re-tune.

## 6. The seesaw thread

The seesaw effect (search oscillation between an ancestor and its subtree
in deep conversions) has exactly two published reduction sites, and both
are closed here:

- **The stay-deeper dial at the threshold site** is implemented (1+ε, §2)
  and its tuning surface is measured-closed (the four legs).
- **The leaf-value site** (Deep df-pn 2017: depth-dependent unsolved-leaf
  `pn/dn`) is **path-dependent at its core** — every faithful mapping lands
  in class (c) or (d) (§3's poisoning argument; the workaround table).
  DeepPN 2015 was not mined separately by recorded scope decision: it is
  best-first, hence excluded by §4. Evidence: [`report9.md`](report9.md);
  [`research_deep_dfpn.md`](research_deep_dfpn.md);
  [`../docs/bibliography.md`](../../bibliography.md) (DeepPN `Cited`).

Even in the paper's home regime the evidence does not favor the leaf-value
site: in Zhang 2017's own head-to-head (8 Connect6 openings, ≤500 k-node
cutoffs, per-position best-of-300 tuning, VCDT confound), best-tuned 1+ε
matches or beats Deep df-pn on node count in 4 of 8 positions. Evidence:
[`report9.md`](report9.md).

**The floor:** the seesaw is attacked where it can be (threshold site,
implemented); the alternative site is contract-excluded and loses its own
published comparison.

## 7. The child-level termination surface

Stopping a child's evaluation early (rather than the parent's frame) maps,
per the mined Henderson 2010 FDFPN extraction, onto four families — none
sound, unmeasured, and unmapped here:

1. **Threshold increments** — implemented (1+ε); the dynamic-δ variant
   degenerates without heuristic initialization and its schedule space was
   closed by plan4. Evidence: [`report5.md`](report5.md) (family 1).
2. **Count-based child limits** (FDFPN; Yoshizoe dynamic widening) — sound
   per the published guarantees but *structurally equivalent to the closed
   partial-sum lever* and **inverted relative to the churn mass**: they
   delay the threshold cuts that carry ~80% of descendant evals, because
   the tail children's 1-eval prices are exactly what lets the frame's Σ
   cut (Henderson's own Observation 3 is the deferral asymmetry). The
   partial-sum lever itself was measured a net loss by `conversion` plan7
   (stress 3.28× evals, re-entry churn 96.3% of the run). Evidence:
   [`report5.md`](report5.md) (family 2);
   [`../conversion/initiative.md`](../conversion/initiative.md) #6.
3. **Correlation/heuristic pruning** (Seo, Schaeffer) — unsound here
   without a domain-proven equivalence class or a heuristic evaluator; the
   same no-heuristic-component blocker that closed EWS. Evidence:
   [`report5.md`](report5.md) (family 3).
4. **Loop-avoidance / terminal-detection / ordering engineering** — already
   covered by the GHI shortcut and existence-query classification, or
   ordering-lever territory (§5). Evidence: [`report5.md`](report5.md)
   (family 4).

**The floor:** the child-granularity termination surface contains no
class-(b) mechanism; what sounds like "evaluate less" collides with the
measured fact that the work mass *is* the priced-in threshold churn (§8).

## 8. The structural characterization of the hard class

The outlier-hard positions — the class all the above levers were aimed at
— are characterized as follows:

- **What it is.** One position family: m20–m23, eight consecutive plies of
  a single known atomic game, material `BNPPPPPPPRR vs PPPPPPR` — **20
  men, pawns present, no castling** — accounting for 58.3% of all outlier
  occurrences and 67% of unique outlier positions across five suites. Four
  of the eight time out at 60 s; difficulty swings by an order of magnitude
  across single plies (m22_white 14.2 M vs m21_white 249.5 M first-outcome
  child evals); m21_white's win is a 477-ply line. The class is deep,
  repetition-dominated, and tree ≫ TT (stress ends at 95.4% TT occupancy
  and still solves exactly at baseline). Evidence:
  [`report2.md`](report2.md); [`report7.md`](report7.md) (occupancy);
  [`report9.md`](report9.md) (regime statement).
- **The negative space — no simplification to exploit.** The searched tree
  never descends into small material: zero `dfpn` frames below 6 men were
  ever entered on m22/stress/dec13; 99.93–99.97% of frame-eval mass sits at
  ≥9 men; harvestable (≤5-men, pawnless, no-castling) frames — the
  invocation surface of any mid-search recognizer — number **exactly 0**.
  The pre-flight detector has no misses at the outlier roots (0/24
  eligible). Evidence: [`report3.md`](report3.md); [`report2.md`](report2.md)
  (feature table).
- **Where the work actually is.** >98% of child evals are unsolved /
  unclassified; ~80% of cumulative descendant evals sit in threshold-cut
  frames (m22 80.30%, stress 81.28%); fast exits (terminal, path-repetition,
  TT-solved) are <1%. Evidence: [`report1.md`](report1.md);
  [`../conversion/initiative.md`](../conversion/initiative.md) #6 (98.9%
  cut mass, plan6 diagnostic).

**The floor:** the hard class is *priced-in churn in material-rich
positions* — it cannot be short-circuited by recognizers (no invocation
sites), ordering (§5), child termination (§7), or threshold reshaping (§2).
What remains is a property of DF-PN threshold dynamics on this class, and
every algorithm-swap candidate that could change those dynamics is excluded
by §1 and §4.

## 9. Known open threads after closure

None of the following is a recommendation; each is recorded as *closed for
now, reopen trigger X, owner Y*.

| Thread | Blocker (why closed for now) | Owner / reopen trigger |
|---|---|---|
| `research` backlog #6 — ML node priors for PNS | No heuristic/NN component exists in a pure solver; the blocker sharpened by the plan5 mining (every learned mechanism surveyed needs an evaluator this solver does not have) | `research` (closed); reopen with a sound pure-solver surrogate or an explicit decision to add one |
| `research` backlog #7 — mating-net recognizers | Zero harvestable subgames (plan3): no ≤5-men pawnless frames are ever entered; the pre-flight detector covers the only eligible roots | `research` (closed); reopen only with a recognizer class that fires at ≥9 men |
| `research` backlog #9 — job-level parallel PNS | Not a node-count lever (wall-time only); the actionable design work is owned by `conversion` #4 | `conversion` #4 (open), jointly with `lean` #2 |
| `research` backlog #13 — frontier prediction priors | Pre-weakened by the ordering oracle floor (lean plan9): the separated signal classes are already rank-optimal; rescue mass concentrates where confidence conditioning was measured non-exploitable (plan8) | `research` (closed); reopen only with a new signal class outside plan8's feature set |
| `conversion` #4 — parallel search design spike | Open, owned jointly with `lean` #2 (parked dormant after its 1.47–1.48× deterministic-ceiling spike; reopen triggers recorded there) | `conversion` + `lean` |
| `lean` #10 — history/killer constant re-tuning | Open, S–M effort, ~0–5% evals estimate; never re-tuned since the GHI/twin removal | `lean` |
| Gao 2021 — true pn/dn in DAGs is NP-hard (`conversion` #5e) | Information-only framing: DAG-aware pn/dn must remain heuristic; no mechanism | `conversion` #5e (open, reading item) |

Bibliography note: the **Open** rows of
[`docs/bibliography.md`](../../bibliography.md) (Saffidine 2011, Young
2016, Čížek 2025, Gao 2021) all point at `conversion` backlogs — this
initiative's closure orphans no bibliography entry.

---

## Evidence gaps

None. Every claim in sections 1–9 traces to an existing report or
extraction; the Phase 0 audit
([`measurements/plan10/claims.md`](measurements/plan10/claims.md)) verified
all 33 claim rows against their targets. Two *honest limits of the record*
are stated in place rather than as gaps: the never-run comparisons of §1
(df-pn vs PDS-PN; repetition-dominated domains) and the metric-population
caveat on the OR-side work-share concentration figures (§5, lean report9 M1
vs the `nn` 90.6% figure). Both are recorded by their owning reports and
are closed threads, not open ones.
