# Report 5: Literature mine — child-level early termination in DF-PN (backlog #5)

Executed 2026-09-20 per `plan5.md`. Mining plan: Phase 0 bounded survey →
one source selected and mined (`research_child_termination.md`) → Phase 2
classification of every surveyed mechanism against the repetition/TT
contracts and the `conversion` #6/#7 deferral-asymmetry test. No `src/` or
`examples/` changes at any point.

## Verdict

**CLOSED (H0).** The published child-level early-termination surface in the
PNS/DF-PN family collapses onto four families, none of which is sound,
unmeasured, and unmapped here:

1. **Threshold increments** (Nagai δ>1, Kishimoto dynamic δ, Pawlewicz & Lew
   1+ε) — the 1+ε second-child rule is implemented (`epsilon_ceil`); the
   dynamic-δ variant degenerates without heuristic initialization (backlog
   #6) and its schedule space was closed by plan4.
2. **Count-based child limits** (Henderson 2010 FDFPN — the mined source;
   Yoshizoe 2008 dynamic widening, closed access) — sound per the published
   guarantees but structurally equivalent to the closed partial-sum sweep
   lever (`conversion` report7): subset-Σ stored bounds, the identical
   re-entry-churn mechanism, and the identical deferral asymmetry — which
   Henderson himself quantifies as Observation 3. Mapped honestly onto this
   solver's incremental full-table fold, the mechanism is *inverted* relative
   to the churn mass: it delays threshold cuts (plan1's ~80% mass) instead of
   accelerating them, because the tail children's 1-eval prices are exactly
   what lets the frame's Σ cut and the parent cut on re-entry.
3. **Correlation/heuristic-based child pruning** (Seo correlated-sibling
   counting, Schaeffer heuristic-threshold PNS) — unsound here without a
   domain-proven equivalence class or a heuristic evaluator; dependency class
   = backlog #6/#7, the same blocker class that closed EWS.
4. **Loop-avoidance / terminal-detection / ordering engineering**
   (df-pn(r), TCA, early terminal detection, killer-tree) — already covered
   by the GHI shortcut + existence-query classification, or ordering-lever
   territory (`lean` #10), or out of scope (`conversion` #2, backlog #7/#8).

Backlog #5 closes with the mapping as the no-go record. No POC proposal; the
sizing sketch in the extraction is archival only.

## Gate decision

| Gate | Criterion | Decision |
|---|---|---|
| OPEN | ≥1 mechanism in class (b), named call site, soundness survives the contract check | **Not met** — zero class-(b) mechanisms (classification table in `research_child_termination.md` §5) |
| CLOSED | All mechanisms in (a)/(c)/(d), or no qualifying source | **Met** — qualifying source found, obtained, and mined; every surveyed mechanism classified into (a), (c), (d), or out-of-scope |
| DEFER | Qualifying source unobtainable / pseudo-code too imprecise | Not triggered for the selected source (Henderson 2010 is open-access and fully pseudocoded); Yoshizoe 2008's closed access is recorded but its mechanism family was classifiable via the mined source |

## Mined source

- **P. T. Henderson (2010), *Playing and Solving the Game of Hex*, Ph.D.
  thesis, University of Alberta** — §5.2.2–5.2.5: Focused DFPN child limit
  `l = base + ⌈fraction × |live children|⌉`, frontier dynamics, Observations
  1–3, Hex tuning (base 1, fraction ≈ 0.2, < 60% of df-pn time; excessive
  pruning worse than none). Extraction:
  `research_child_termination.md`; vendored PDF:
  `measurements/plan5/henderson2010_playing_solving_hex.pdf`.
- Corroborating: Gao, Müller, Hayward (IJCAI-17, FDFPN-CNN — rule restated as
  Eqs. (1)–(3), learned components out of scope = backlog #6); ICGA-2012
  survey §7 (classification backbone). Yoshizoe 2008 noted closed-access.

## Classification summary

| Mechanism | Class |
|---|---|
| 1+ε second-child threshold | (a) implemented |
| δ>1 / dynamic-δ threshold increments | (d) subsumed (needs heuristic init; schedule space closed by plan4) |
| FDFPN child limit (mined) | (d) structurally equivalent to closed partial-sum sweep lever; inverted vs. churn mass |
| Dynamic widening (Yoshizoe 2008) | (d) same class; also closed-access |
| Correlated-sibling counting (Seo) | (c) unsound here (needs domain equivalence proof; ML variants = #6) |
| Heuristic-threshold PNS (Schaeffer 2005) | (c) not viable here (needs heuristic evaluator; EWS-class blocker) |
| λ-family / PN² / PDS-PN / PDS-swap / Kawano / killer-tree | out of scope (#7, #8, `conversion` #2, `lean` #10) |
| df-pn(r) / TCA / terminal detection | (a) covered by existing GHI shortcut / existence queries |

## Phase 0 evidence

`measurements/plan5/query_log.md` (13 queries/endpoints) and
`measurements/plan5/shortlist.md` (13-source shortlist with
why-not-selected). Headline negatives for the record: Nagai's dissertation
not obtainable (PDS's cutoff rule covered via secondary sources); "Deep df-pn
and Its Efficient Implementations" closed; "partial expansion PNS" returns
nothing relevant; Čížek 2025 full-text grep shows no child-level cutoff
content.

## Deliverables

- `research_child_termination.md` — extraction (summary, rule, mapping to
  verified call sites, soundness check, classification, sizing sketch).
- `measurements/plan5/` — query log, shortlist, vendored PDFs.
- `docs/bibliography.md` — Henderson 2010 added (**mined**); Gao et al. 2017,
  Yoshizoe 2008, ICGA-2012 survey, Winands PDS-PN, PNS-variants chapter added
  or annotated (**cited**).
- `initiative.md` — backlog row #5 closed, History entry.

## Verification

- `src/` and `examples/` untouched: `git status` shows only `docs/` changes.
- No benchmark runs; no claim in the extraction rests on a new measurement
  (all numbers cited from plan1/plan4/conversion report7 records or the
  sources themselves).
- Every claimed code site read this session: `core.rs` (frame loop, threshold
  cut, `epsilon_ceil`, `sort_moves` call), `children.rs`
  (`evaluate_all_children`, `evaluate_child`), `selection.rs`
  (`selection_for_child`, `is_solved_by_children`, `best_and_second_unsolved`),
  `history.rs` (`sort_moves`).
- Housekeeping gate: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` — run after the docs-only change; nothing was
  expected to move (hygiene check per the Boy Scout principle).

## Next steps

Backlog #5 closed. Next literature target per the plan's CLOSED consequence:
#6 (ML node priors) or #7 (mating-net recognizers) — both carry a known
transfer blocker (no heuristic/NN component in a pure solver), which this
mining sharpened; #8 (PDS/PN² algorithm swap) remains genuinely separate —
note that PDS's *cutoff rule* was classified here (subsumed), so #8's open
question is exclusively the full-algorithm replacement scaling comparison.
