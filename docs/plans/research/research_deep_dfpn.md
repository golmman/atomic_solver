# Research: Deep df-pn (Zhang, Iida, van den Herik 2017) — depth-dependent unsolved-leaf pn/dn (`research` backlog #15)

Mined 2026-09-21 by `plan9.md` (literature target #15: seesaw-effect
reducers). Primary source vendored in-repo:
`docs/theory/deep-dfpn-2017/deep-dfpn-2017.md` (OCR extraction of the ACG
2017 paper, LNCS 10664, pp. 73–89); the DeepPN 2015 predecessor is
`docs/theory/deep-pns-2015/` and is **not** mined separately per the plan's
scope decision (best-first frontier family closed by plan6; RAM = TT only
fatal — the 2017 paper's own §1 records DeepPN's storage and update costs).
All paper claims below cite the source's own section/definition/table
numbering. All code-site claims were verified by reading the files on
2026-09-21 (see `measurements/plan9/notes.md` for the quoted excerpts).

## 1. Summary

Deep df-pn modifies df-pn at exactly one site: **the (pn, dn) assigned to an
unsolved leaf**. Where df-pn assigns 1 (the paper's case (c): "when the
value of n is unknown"), Deep df-pn assigns

```
D_dfpn(depth) = E^(D − depth)   (depth < D, E > 0)
              = 1               (depth ≥ D)
              = 0               (E = 0, depth-first degenerate)
```

(Definition 1) with two integer parameters: **E** ("a threshold of branch
size") and **D** ("a threshold of depth", counted from the root). Internal
nodes keep the standard OR/AND folds (min / sum). Table 1 gives the behavior
map: E = 0 is depth-first search, E = 1 or D ≤ 1 is plain df-pn, E > 1 ∧
D > 1 is "intermediate" between depth-first and df-pn. **Selection and
threshold recursion are untouched** — the stay-deeper bias arises purely
because the standard folds propagate the inflated leaf values: expanding a
node at depth < D replaces its leaf value E^(D−depth) with the child fold
E′·E^(D−depth−1), which is *smaller* whenever E′ < E, so an expanded subtree
becomes more attractive and the search keeps descending instead of
seesawing back to an ancestor (Theorem 1). The seesaw effect is the paper's
name for what plan1 measured in this solver as threshold-cut churn:
"frequently going back to the ancestor nodes for selecting the most-proving
node" (§3.1).

The paper is explicit that this is one of two ways to control the same
behavioral dial: "changing the search behavior of df-pn can be implemented
by two methods: (1) changing the thresholds … (such as 1 + ε trick); (2)
changing the proof number and disproof number of unsolved nodes. Deep df-pn
implements the method (2)." (§4). The threshold site (method 1) is the
implemented `epsilon_ceil` mechanism here.

**Theorem 1's assumptions are load-bearing and untypical of our regime:**
the expansion inequality `Σ_children D_dfpn(d+1) < D_dfpn(d)` requires
E′ < E (the expanded node has below-average branching), and the argument is
stated over a pure tree — it "says nothing about" transpositions (no TT in
the model), repetitions, or re-visited nodes. The authors themselves flag
the regime question in §7: Connect6 "is a game with an unbalanced game tree
(with a large number of sudden deaths)", and they recommend testing
"balanced game tree (fix-depth tree or nearly fix-depth tree)" games.

## 2. Background: the rule as published

- **Definition 1** (§4): the two-parameter leaf function above. E and D live
  on ℕ; the published sweep is E ∈ [1, 20], D ∈ [1, 15] (§6.1).
- **Where it enters**: only case (c) leaves (unknown value). Terminal leaves
  keep (0, ∞) / (∞, 0). Internal folds are the textbook min/sum.
- **The complete algorithm** is not in the paper; it is "accessible on the
  website" (§4 footnote 1, jaist.ac.jp — not vendored; the definition plus
  Table 1 is sufficient to characterize the mechanism, and the paper states
  everything else is standard df-pn).
- **§6.3 comparison with 1+ε**: the paper restates the 1+ε threshold rule
  (`n₁.thδ = min(n.thδ, ⌈n₂.δ(1+ε)⌉)`) and compares best-tuned instances of
  both on the same 8 positions, parameters tuned independently (ε swept
  0.05–15 in 0.05 steps). It also observes 1+ε exhibits the "noise effect"
  (huge time jumps from tiny ε changes) — a phenomenon this solver measured
  from the inside as trajectory chaos (plan4/plan8).
- **§6.4 parameter search**: per-position hill-climbing over (E, D) with a
  node-number cutoff; average 234.4 s of tuning per position, average 3.3%
  worse than the best-case grid point (Table 3).

## 3. Evidence table (normalized)

All numbers from Table 2 (best case per method, per position) and Table 3
(hill-climbing), §6. Node numbers include repeatedly traversed nodes and
VCDT sub-solver nodes. Bracketed values are the paper's reduction vs plain
df-pn. "1+ε better?" flags positions where the *threshold-site* mechanism —
the one this solver already implements — matched or beat Deep df-pn on
nodes.

| Pos | Deep df-pn nodes (red.) | E, D | 1+ε nodes (red.) | ε | 1+ε better on nodes? |
|-----|--------------------------|------|-------------------|------|------|
| 1 | 5,568 (96.5%) | 17, 4 | 5,633 (96.4%) | 2.85 | ~tie (−0.1 pp) |
| 2 | 45,300 (33.7%) | 7, 6 | 38,948 (43.0%) | 4.05 | **yes** (+9.3 pp) |
| 3 | 21,157 (0.7%) | 5, 4 | 21,309 (0%) | 0.05 | ~tie |
| 4 | 99,073 (17.1%) | 8, 6 | 95,472 (20.2%) | 0.25 | **yes** (+3.1 pp) |
| 5 | 163 (99.8%) | 18, 2 | 82,777 (5.7%) | 0.05 | no (Deep +94.1 pp) |
| 6 | 47,213 (8.6%) | 14, 4 | 46,255 (10.4%) | 0.15 | **yes** (+1.8 pp) |
| 7 | 74,061 (45.9%) | 7, 4 | 143,609 (−4.9%) | 0.05 | no (Deep +50.8 pp) |
| 8 | 203,188 (13.1%) | 5, 4 | 187,198 (20.0%) | 0.25 | **yes** (+6.9 pp) |
| Avg | 61,965 (43.5%) | — | 77,650 (29.2%) | — | 1+ε ≥ Deep on 4/8 positions |

What the comparisons do **not** cover:

- **No TT.** The df-pn model of §2.2 and Theorem 1 has no transposition
  table; the implementation section (§6.1) mentions a TT only inside the
  *VCDT sub-solver*, not in Deep df-pn itself. The mechanism's interaction
  with TT-stored bounds — the crux for this solver — is unexamined by the
  source.
- **No repetition handling.** Connect6 placements cannot repeat; the
  repetition-dominated structure of our stress class is absent.
- **Per-position best-case tuning.** Every Deep df-pn row is the best of a
  300-point (E, D) grid; every 1+ε row the best of a 300-point ε sweep. No
  default parameter is shown to transfer across positions: the winning
  (E, D) ranges over E ∈ [3, 18], D ∈ [2, 6] (Tables 2–3).
- **A relevance-zone/VCDT sub-solver does unmeasured work.** Defender moves
  are generated from derived relevance zones (§5.2) and attacker moves are
  cut to the top-5 by a heuristic (§6.1) — a second, fixed-widening
  mechanism inside every measured node count.
- **Regime.** Positions are mostly 4-move Connect6 openings; node counts run
  163–203 k with solver cutoffs of 500 k / 160 k nodes (§6.1). Our gate
  objects are 14.2 M (`m22_white`) and 249.5 M (stress) first-outcome child
  evals — two to three orders of magnitude larger, repetition-dominated,
  with a 60+-ply conversion proof shape the authors themselves exclude from
  their claim's comfort zone (§7).

## 4. Mapping to this solver (all sites verified — quotes in `measurements/plan9/notes.md`)

**(i) Where unsolved leaves get their initial pn/dn.** Two sites:

1. `src/search/dfpn/children.rs::evaluate_child`, final `else` branch: an
   unsolved child receives either TT-reused bounds
   (`(e.pn, e.dn)` when `e.outcome.is_none() && e.pn > 0 && e.dn > 0 &&
   e.remaining_depth != u32::MAX && e.remaining_depth <= child_max_depth`)
   or the neutral fallback `(1, 1)`. The `(1, 1)` fallback is df-pn's
   unsolved-leaf value — exactly the quantity `D_dfpn(depth)` would replace.
2. `src/search/dfpn/core.rs`, the `max_depth == 0` bounded-leaf branch:
   `self.tt.store(tt_key, Move::NONE, u8::MAX, 0, None, 1, 1, 0, 0)` — the
   frontier leaf stores (1, 1) into the TT.

**(ii) Does a depth-like quantity exist at those sites?** Yes — the mapping
does **not** fail on "no depth source":

- `self.path_stack.len()` is the path depth from the current chunk root. It
  is read in `evaluate_child` already (`scratch_depth = self.path_stack.len()`)
  and in `dfpn` (`frame_depth = self.path_stack.len()` after `path_push`).
- `max_depth` / `child_max_depth` (the current bounded chunk's remaining
  budget) are arguments at both sites.
- `pos.board().rule50()` (the halfmove clock) is a *position function* — the
  halfmove clock is part of the Zobrist key — but it measures depth-since-
  last-irreversible-move, not depth-from-root.

**(iii) Where the values feed selection and thresholds.**
`src/search/dfpn/selection.rs` folds the `ChildInfo` table (OR:
`pn = min c.pn`, `dn = Σ c.dn` saturating; AND mirrored; `best_and_second_unsolved`
argmin over pn/dn) — unchanged by the mechanism, but its inputs inflate.
`src/search/dfpn/core.rs` threshold recursion: OR child gets
`new_th_pn = min(th_pn, epsilon_ceil(second_pn))` and
`new_th_dn = th_dn − dn + child_dn`; AND mirrored. Under Deep df-pn the
child_dn/child_pn passed into the recursion and the Σ folds all carry
E^(D−depth) terms — the paper's own arithmetic (§4 internal-node definition)
works identically, so the mechanism maps mechanically.

**(iv) Where the results are stored.** `core.rs` frame exit:
`(store_pn, store_dn) = (pn.max(1), dn.max(1))` for unsolved frames, with
`store_remaining_depth = max_depth`, into `TtEntry { key, best_move,
best_child, work, outcome, pn, dn, depth, remaining_depth }`
(`src/search/tt/entry.rs`), probed back in `evaluate_child` under the guard
quoted in (i). This is where the path-dependence lands (§5.1).

## 5. Contract check

### 5.1 TT path-independence — the crux. **Fails for the faithful mechanism.**

The documented contract (`src/search/tt/`, per AGENTS.md: "path-independent
base entries; repetition-dependent results are not cached") is embodied in
`TtEntry`: one entry per position hash, holding `outcome: Option<Outcome>`,
`pn`, `dn`, `depth`, `remaining_depth` — all today pure functions of the
subtree, because every leaf contributes the position-only value (1, 1)
(or a previously stored subtree summary).

Under the faithful mechanism (leaf value keyed to `path_stack.len()`), a
frame's unsolved bounds `(pn.max(1), dn.max(1))` stored at frame exit
sum D_dfpn values along *the path the frame happened to sit on*: the same
position reached by two different lines produces different stored
`(pn, dn)`. `TtEntry.pn/dn` stop being functions of the position — the
contract breaks at the exact store line quoted in (iv). The probe side
compounds it: `evaluate_child` reuses `(e.pn, e.dn)` on whatever path the
probe happens to occur on (the guard checks only `remaining_depth`), so a
foreign-path valuation is imported as if it were the subtree's own — which
also voids Theorem 1's premise *within* the mechanism: the stay-deeper
inequality assumes consistent D_dfpn values along the path, and a
TT-imported bound computed at another depth is arbitrary noise with respect
to that inequality. The mechanism's intended effect does not survive its
own caching.

Workarounds, each fatal or mutative:

- **Store (1, 1) always, never deep-derived bounds** — discards the
  unsolved-bounds reuse that is the search's working set; the direct analog
  (plan7 V1, dropping stored unsolved bounds) timed out stress and regressed
  m22 +3400%.
- **Tag entries with the path depth they were computed at, reuse only on
  equal depth** — needs a new `TtEntry` field, and collapses transposition
  reuse: re-entries at a different path depth (the majority of re-entries in
  a 249 M-eval search) become cold. No evidence anywhere that the residual
  benefit pays for this.
- **Replace depth with a position-only proxy** (`rule50` clock, material
  count) — restores path-independence by construction, but is no longer the
  published mechanism: root-depth does not appear in the position, and the
  proxies point the wrong way (a high halfmove clock marks *quiet deep*
  lines, not shallow ones; D_dfpn(rule50) inflates exactly the conversion
  endings where the clock is high, inverting the intended bias). Class (d):
  a mutated mechanism with zero published evidence.

### 5.2 GHI / first-player-loss shortcut. Compatible, with a note.

The shortcut (`suppress_draw` on `repetition_seen`, `core.rs`) is
order-independent in correctness: it suppresses caching of a Draw whose
proof used a path repetition, whatever search order produced it. Deep
values would change *which lines are entered* (hence which repetition
classes are encountered and which draws get suppressed), but that is a
trajectory change, not a soundness change — the same latitude the 1+ε knob
already has. The per-run `RepetitionCache` keys on
(position, ancestor-repetition context) and returns only Draws under an
identical ancestor set; leaf values do not touch it. No break.

### 5.3 RAM = TT only. Compatible.

Leaf values computed from existing frame state (`path_stack.len()`,
`max_depth`) add no structure — this is the advantage over DeepPN 2015,
whose open frontier is plan6-class fatal. The only new memory would be the
path-depth tag workaround in §5.1 (two extra bytes per entry), which fails
on economics, not memory.

### 5.4 `ProofEvent` neutrality. Compatible.

Leaf values drive selection and cuts only; `is_solved_by_children` decides
outcomes from child *outcomes*, not from pn/dn magnitudes. A deep-value arm
changes when frames exit, never what is proven; emission sites are
untouched. (Corollary worth stating: the mechanism is *soundness-neutral* —
its risks are contractual and economic, not correctness.)

### 5.5 Refinement rounds / `--refine-cap` / child-eval-budget. Unaddressed by the source; a consistency hazard.

The paper runs one unbounded df-pn search; this solver runs iterative
bounded chunks (refinement rounds with per-round work caps, a cumulative
child-eval budget, and `max_work` chunking) whose correctness story is
"the next round grows past the previous round's stored bounds". A
depth-dependent leaf value shifts *between rounds*:

- Keyed to path depth: stable per path, but the stored-bounds problem of
  §5.1 means round N re-imports round N−1 bounds computed under whatever
  paths round N−1 happened to take — cross-round non-monotone guidance.
- Keyed to remaining budget (`max_depth`): changes for the same position
  every round, so the (1,1)-analog leaf constant is not a fixed point and
  the round-to-round bound-growth assumption degrades into noise.

Neither variant has any analog in the paper; making one consistent is
new-algorithm design, not mining. Additionally, an env-gated arm would
need to hold its values stable under the deterministic work-cap arithmetic
(`set_refine_cap_factor`, `set_child_eval_budget`) — feasible, but there is
no published guidance on what the values should be per round.

### 5.6 1+ε interaction. Composes arithmetically, overlaps behaviorally.

Arithmetically the two compose cleanly: ε acts at the threshold site
(`epsilon_ceil(second_pn/dn)` on the second-best child), deep values at the
leaf site feeding the folds and the passed `child_dn/child_pn`; no δ-sum
identity is violated. But behaviorally both are the same dial — the paper
says exactly this (§4, methods (1) and (2) for "changing the search
behavior") — so their gains overlap, and the tuned-parameter spaces would
confound (the paper tuned both independently per position, §6.3). On this
solver's evidence the dial itself is measured-closed: plan4 (no schedule
beats the constant; trajectory chaos), plan8 (no node-local conditioning
converts to a win), with the four ε closure legs. A second knob on the same
dial inherits that prior.

## 6. Phase 2 classification

| Mechanism / variant | Class | Reasoning |
|---|---|---|
| **Faithful Deep df-pn** (leaf value = D_dfpn(path depth), TT kept as-is) | **(c) contract-breaking** | Stored unsolved bounds `(pn.max(1), dn.max(1))` become path-relative — breaks the `src/search/tt/` path-independent base-entry contract at the `core.rs` frame-exit store; the `evaluate_child` reuse guard then imports foreign-path valuations, voiding Theorem 1's own consistency premise. §5.1. |
| Path-depth variant + path-depth-tagged entries (reuse only at equal depth) | **(c)** | New `TtEntry` field; collapses transposition reuse (the dominant resolution path in the 250 M-eval regime); no published evidence the residual benefit can pay. §5.1. |
| Never store deep-derived bounds (store (1,1)) | **(c)** | Discards unsolved-bounds reuse — plan7 V1 measured the direct analog: stress timeout, m22 +3400%. §5.1. |
| Position-only proxies (rule50 clock, material) | **(d)** | Mutates the mechanism (root depth is not a position function; proxies point directionally wrong); zero published evidence; would still be a second knob on the behavior dial plan4/plan8 closed. §5.1. |
| E = 0 degenerate arm (depth-first) | **(d)** | The paper's own measurement: "far more time than the original df-pn" (§6.2); corresponds to discarding pn/dn guidance entirely. |
| The *behavioral target* (stay-deeper bias against seesaw churn) | **(a) covered** | The implemented 1+ε threshold recursion is the paper's method (1) for the same dial (§4); its tuning surface is measured-closed on this solver (plan4 Phase 0/1, plan8 arms; ε=0.375 Pareto note), and the iterative bounded refinement already forces depth-first deepening. |
| Evidence transfer to the stress class | **(d)** | Regime: Connect6 4-move openings, ≤ 500 k-node cutoffs, no TT/repetitions in the mechanism, VCDT relevance zones + top-5 attacker widening confounding every number; head-to-head vs 1+ε is 4/8 positions in 1+ε's favor on nodes (Table 2), with the average carried by two outlier positions (5, 7). Theorem 1 assumes no transpositions and E′ < E. §3, §2. |

**Verdict: no variant lands in class (b). H0 holds; backlog #15 closes.**
The seesaw-thread no-go record for #16 (`structural_floor.md`): the
stay-deeper dial exists in this solver at the threshold site (1+ε) and is
measured-closed; the leaf-value site (Deep df-pn) is path-dependent at its
core — the faithful mapping breaks the path-independent TT contract at the
`core.rs` frame-exit store, and every path-independent workaround either
mutates the mechanism beyond its published evidence or destroys the
unsolved-bounds reuse that plan7 measured as the search's working set.

## 7. Sizing sketch (for the record — no POC proposed)

Had a class-(b) variant survived, the POC contract would be: env-gated
`D_dfpn` arm on the `evaluate_child` fallback and the `max_depth == 0`
store, gate object = stress first-outcome `child_evals`, controls =
`m22_white`/`dec13`/`dec10`, bit-identical gate-off anchor, and a
pre-registered requirement that the arm hold the ≥ 10% stress bar while the
published default (E, D) is unknown for our regime — meaning a hill-climbing
tuning pass per gate object (the paper's §6.4, ~234 s per position at
500 k-node cutoffs, extrapolating badly to 249 M-eval runs). Estimated
2–3 sessions with negative expected value given §6. **Not recommended;
archived with the (c)/(d) classifications above.**
