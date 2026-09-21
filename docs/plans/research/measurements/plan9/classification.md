# plan9 — (a)–(d) classification

Per `plan9.md` Phase 2 and the plan6 classification vocabulary. Full
reasoning in `../research_deep_dfpn.md` §5–§6; code excerpts in
`code-sites.md`.

| # | Mechanism / variant | Class | One-line reason |
|---|---------------------|-------|-----------------|
| 1 | Faithful Deep df-pn: unsolved-leaf value `D_dfpn(path depth) = E^(D−depth)` at the `children.rs::evaluate_child` `(1, 1)` fallback and the `core.rs` `max_depth == 0` store, TT kept as-is | **(c) contract-breaking** | `TtEntry.pn/dn` become path-relative at the `core.rs` frame-exit store (`(pn.max(1), dn.max(1))` sums D_dfpn along the frame's path); the `evaluate_child` reuse guard (no path-depth condition) then imports foreign-path valuations, voiding Theorem 1's consistency premise. Breaks the `src/search/tt/` path-independent base-entry contract. |
| 2 | Variant 1 + path-depth tag per entry, reuse only at equal depth | **(c)** | Needs a new `TtEntry` field (none exists; two-slot bucket layout in `table.rs`); collapses transposition reuse — the dominant resolution path in the 14 M–250 M-eval regime; no published evidence the residue pays. |
| 3 | Variant 1 but never store deep-derived bounds (store (1, 1) always) | **(c)** | Discards unsolved-bounds reuse; direct analog measured in plan7 V1 (dropping stored unsolved bounds): stress timeout, m22 +3400%. |
| 4 | Position-only depth proxies (halfmove clock, material count) in `D_dfpn` | **(d) evidence-absent** | Mutates the mechanism: root depth is not a position function; the clock measures depth-since-last-irreversible-move (directionally wrong — inflates quiet conversion endings); zero published evidence; still a second knob on the stay-deeper dial plan4/plan8 closed. |
| 5 | E = 0 degenerate arm (pure depth-first) | **(d) evidence-against** | The paper's own §6.2: "far more time than the original df-pn"; equivalent to discarding pn/dn guidance. |
| 6 | The behavioral target (stay-deeper bias vs seesaw churn) as such | **(a) structurally covered** | The implemented 1+ε threshold recursion (`epsilon_ceil` in `core.rs`) is the paper's method (1) for the same dial (§4: "two methods"); its tuning surface is measured-closed (plan4: no schedule beats the constant, trajectory chaos; plan8: no node-local signal converts; four ε closure legs); iterative bounded refinement already forces depth-first deepening. |
| 7 | Published evidence transfer to the stress class | **(d)** | 8 Connect6 positions (mostly 4-move openings), ≤ 500 k-node cutoffs, no TT/repetitions in the mechanism's model, VCDT relevance zones + top-5 attacker widening confounding all numbers; Table 2 head-to-head vs best-tuned 1+ε: 1+ε matches or beats Deep df-pn on node count in 4/8 positions, average carried by positions 5 and 7; Theorem 1 assumes no transpositions and E′ < E; §7 itself excludes unbalanced-tree regimes from the claim's comfort zone. |

**Verdict: no mechanism in class (b). H0 holds. Gate: CLOSED** — backlog
#15 closes with this table as the seesaw-thread no-go record; the next
(and likely last) plan is #16, the `structural_floor.md` consolidation.
