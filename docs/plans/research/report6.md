# Report 6: Literature mine — PDS / PN² algorithm-swap scaling vs DF-PN+ (backlog #8)

Executed 2026-09-20 per `plan6.md`. Mining plan: Phase 0 bounded survey →
one source selected and mined (`research_alternative_algorithms.md`) →
Phase 2 classification of every whole-algorithm variant against the GHI /
path-independence / RAM contracts. No `src/` or `examples/` changes at any
point.

## Verdict

**CLOSED (H0).** The published record does not show PDS, PN², or any
descendant scaling better than df-pn/DF-PN+ in the regime that matters
here — deep, TT-bounded AND/OR solving — and no variant survives the
contracts with evidence of a win:

1. **PDS** — class (d), evidence-against. The only two direct
   df-pn-vs-PDS comparisons ever published (Pawlewicz & Lew 2007: Atari
   Go TT-size sweep; 286 hard LOA positions) favor df-pn by 2.6–4.5× in
   solving time, with the largest margins exactly in the "search tree ≫
   TT" regime that defines our stress class. The swap's structural deltas
   against the implemented DF-PN+ are the +1 threshold schedule (the ε→0
   corner of the implemented 1+ε rule; schedule space closed by plan4),
   delayed evaluation (the published 7–8× node-generation overhead), and
   TT-stored per-node thresholds (constant-factor layout). Published PDS
   also ignores GHI outright.
2. **PN²** — class (c), contract-breaking. The level-2 best-first
   frontier is non-TT memory by construction (fatal against RAM = TT
   only), and the published record independently shows PN²'s solving
   performance collapsing under memory restriction — the ICGA-2012
   survey's regime statement ("for larger problems df-pn may be the best
   choice") names our class.
3. **PDS-PN / DFPN-PN two-level hybrids** — class (c)+(d). Same level-2
   memory problem; 3.7× node inflation vs PDS from the repeated level-2
   rebuilds; and the one comparison that could justify the hybrid (vs
   df-pn on equal hardware) has never been run — the mined chapter
   records this gap explicitly, and the ICGA-2012 survey concedes a
   comprehensive variant study "is sorely missing".

The lock-in cost is quantified, not assumed: **no published head-to-head
favors a replacement algorithm over df-pn in any regime**, and the
repetition-dominated comparison (where the historical df-pn displacement
happened via solvable loop handling) was never run by anyone. The
DF-PN+ commitment stands on the evidence, with the honest caveat that the
evidence base is thin in exactly our regime (thin-but-one-sided: every
measured data point favors df-pn).

## Gate decision

| Gate | Criterion | Decision |
|---|---|---|
| OPEN | ≥1 variant in class (b), named call-site mapping, contract-check pass, comparable-regime evidence | **Not met** — zero class-(b) variants (classification in `research_alternative_algorithms.md` §6) |
| CLOSED | All variants in (a)/(c)/(d), or Phase 0 finds no qualifying source | **Met** — qualifying source found, obtained, and mined (van den Herik & Winands chapter); PDS, PN², PDS-PN, DFPN-PN all classified; incumbent 1+ε df-pn+ confirmed as (a) |
| DEFER | Decisive comparison unobtainable / no reproducible source | Not triggered — the decisive comparisons *are* in the record and uniformly favor df-pn; the true gaps (df-pn vs PDS-PN; repetition-dominated domains) are recorded as such rather than as blockers |

Backlog #8 closes with the head-to-head evidence table
(`research_alternative_algorithms.md` §3) as the lock-in-cost record.

## Mined source

- **H. J. van den Herik, M. H. M. Winands, *Proof-Number Search and its
  Variants* (chapter)** — §§2–8: PDS threshold rules (Eqs. (8)–(9)),
  NegaPDS/NegaPDS-PN pseudo-code, PN² construction (Eqs. (10)–(11)),
  LOA comparison tables (Tables 3–9, memory curves), §7 df-pn-vs-PDS
  ratios (P&L Table 10) + the recorded df-pn-vs-PDS-PN gap.
  Extraction: `research_alternative_algorithms.md`; vendored:
  `measurements/plan6/vanherik_winands_pnchapter.pdf`.
- Corroborating: Pawlewicz & Lew 2007 §4 (primary head-to-head data;
  in-repo `docs/plans/dfpn/epsilon.pdf` — its §4 PDS comparison was not
  covered by the earlier 1+ε extraction), Winands 2002 CG paper (GHI note,
  memory curve), ICGA-2012 survey (regime statement, threshold-control
  catalogue, missing-study concession), Sakuta & Iida 2001 (secondhand
  PDS-overhead figure).

## Classification summary

| Variant | Class |
|---|---|
| PDS (whole algorithm) | (d) evidence-against (2.6–4.5× head-to-head margins vs df-pn; deltas subsumed/closed/published-overhead) |
| PN² | (c) contract-breaking (RAM = TT only fatal; published memory collapse) |
| PDS-PN | (c) contract-breaking + (d) evidence-absent (level-2 frontier; no df-pn comparison exists) |
| DFPN-PN (chapter future-work) | (c) contract-breaking, unpublished |
| df-pn+ with 1+ε (incumbent) | (a) implemented |
| Two-level TT replacement schemes | (d) evidence-against (shogi-community consensus; corroborates the single-table design) |

## Phase 0 evidence

`measurements/plan6/query_log.md` (10 queries/endpoints) and
`measurements/plan6/shortlist.md` (10-source shortlist). Headline records:
all three plan5-identified author-copy sources obtained and vendored
(Winands 2002, PNS-variants chapter, ICGA-2012 survey); Pawlewicz & Lew
2007 found already vendored in-repo; Nagai 2002 and Breuker 1998 confirmed
unobtainable (content superseded by the obtained sources); no post-2012
head-to-head exists (arXiv sweep + the survey's own concession).
Semantic Scholar rate-limited (429); search engines returned shells —
documented, non-blocking given direct fetches succeeded.

## Deliverables

- `research_alternative_algorithms.md` — extraction (summaries, published
  forms, normalized 9-row head-to-head evidence table, mapping to verified
  call sites, contract check, classification, archival sizing sketch).
- `measurements/plan6/` — query log, shortlist, three vendored PDFs.
- `docs/bibliography.md` — van den Herik & Winands chapter upgraded to
  **mined**; Winands 2002, ICGA-2012 survey, Pawlewicz & Lew 2007
  re-annotated (**cited** with plan6 pointer); Sakuta & Iida 2001 added
  (**cited**, secondhand).
- `initiative.md` — backlog row #8 closed, History entry.

## Verification

- `src/` and `examples/` untouched: `git status` shows only `docs/`
  changes; no benchmark runs (the plan's claims rest on the sources'
  published numbers and prior plan records only).
- Every claimed code site read this session: `src/search/dfpn/core.rs`
  (`dfpn` frame signature with `th_pn/th_dn`, the both-thresholds exit,
  the `(np, nd)` epsilon_ceil re-entry arithmetic, `epsilon_ceil`,
  `sort_moves` call), `children.rs` (`evaluate_all_children`,
  `evaluate_child`), `selection.rs` (`select_from_children`,
  `best_and_second_unsolved`, `is_solved_by_children`), `tt/` entry shape,
  `mod.rs` refinement-round state.
- Bibliography and backlog updates cross-checked against this report.
- Housekeeping gate: `cargo fmt --check`, `cargo clippy --release
  --all-targets`, `make test` (hygiene check per the Boy Scout principle;
  nothing was expected to move).

## Next steps

Literature targets #6 (ML node priors) and #7 (mating-net recognizers)
remain open but pre-weakened (no-heuristic-component transfer blocker,
sharpened by plan5; zero harvestable subgames, plan3). With #8 closed, the
backlog's live research surface shifts to the POC candidates (#12 TT
eviction, #13 frontier priors — both with their documented
pre-weakenings) or a new target; the next plan picks per the CLOSED
consequence of `plan6.md`.
