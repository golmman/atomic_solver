# Plan 1 Report — Research reading round: parallel PNS (2025-era)

Executed 2026-09-21 per `plan1.md`. **Docs-only plan confirmed**: no
`src/` change (`git status` clean except `docs/bibliography.md`,
`docs/plans/parallel/initiative.md`, and the new files), no benchmark
runs, no drift protocol (nothing to drift).

## Papers actually read (versions, page counts)

| Paper | Version read | Pages | Vendored |
|---|---|---|---|
| Čížek, Balko, Schmid 2025 (*Massively Parallel Proof-Number Search for Impartial Games and Beyond*) | **arXiv:2511.10339v2** (9 Feb 2026; AAAI-26 copyright line) — v2, not the v1 the bibliography entry was opened against | 19 | `docs/theory/pns-pdfpn-2025/pns-pdfpn-2025.pdf` |
| Saffidine, Jouandeau, Cazenave 2011 (ACG 2011 Tilburg, LNCS, pp. 196–207) | **Author copy via HAL** (`hal.science/hal-01499675`, deposited 31 Mar 2017); Springer paywalled; open-access status "CLOSED" per Semantic Scholar. HAL anti-bot only passes via the Wayback Machine | 13 (1 HAL cover + 12) | `docs/theory/ppn2-2011/ppn2-2011.pdf` |
| Young & Hayward 2016 (*A Reverse Hex Solver*) | **arXiv:1707.00627v1** (26 Apr 2017) | 13 | `docs/theory/solrex-2016/solrex-2016.pdf` |

Version mined is noted per file (§0 of each). Cross-checks used:
`dfpn/research_parallel.md` (Kaneko), `dfpn/research_ghi_journal.md`
(soundness template), `lean/report7.md` (OR/AND split, deterministic
no-go), Čížek's Related Work (for the SPDFPN and PPN₂ scaling figures).

## Deliverables

- `research_cizek2025.md` — two-level decomposition (master/frontier
  jobs + Kaneko PDFPN workers), shared worker info (master key-result
  DB of Grundy numbers + lock-protected worker TT), scaling curve and
  explicit 4–16 core extrapolation, option-D mapping + table row.
- `research_jlpns.md` — the paper proposes **PPN₂** (JLPNS extension
  with PN-search jobs), not JLPNS proper (Wu et al.); job/ownership
  model, load balancing, portfolio question, option-B/C verdict +
  table row.
- `research_solrex.md` — concurrency model (thin in-paper; delegated
  to SPDFPN 2014), low-core measurements, domain transfer risks,
  option-A verdict + table row.
- `parallel/initiative.md` — cross-paper comparison paragraph appended
  to the option-space section; backlog #1 → mined; backlog #2 enriched
  with synthesis pointers and the spike order.
- `docs/bibliography.md` — three entries flipped **Open → Mined**.

## Per-paper verdicts and the surviving-option recommendation

- **Čížek 2025 → conditional GO for the job-level skeleton (C),
  NO-GO for D's shared-DB layer.** In its own small-pool data the
  optimal configuration has `threads = 1` — the shared-TT second level
  engages only at ≥128 cores; the near-linear 4–16 core numbers
  (4.34×/4, 16.22×/16) come from job-level parallelism over persistent
  private-state workers. The master-level shared database — the
  design's biggest lever — carries *path-independent by rule* key
  results (Grundy numbers); our only such class (decisive Win/Loss) is
  banned from cross-worker reuse (plan10 non-goal) and our draws are
  path-dependent ⇒ no safe payload. Work inflation 0.94→2.50 (1→1024
  cores); PNS-PNS (state discarded per job) falls *below* sequential.
- **JLPNS/PPN₂ → GO for option C, B as degenerate adjunct.** Not a
  portfolio in disguise: central master-owned frontier tree, private
  client searches, partial pn/dn feedback, reservation flags (no
  virtual loss — 0/∞ stay attractors). No shared TT anywhere; 18.3×/64
  clients, 7.2×/16, ≈0.5×/client at small pools, ~2× work inflation at
  64. Fits our AND side (~60% of work, lean report7) as full-solve
  jobs; the OR-early-exit tail is the unmeasured risk.
- **Solrex → weak GO for A as a second stage.** Best few-thread
  shared-TT figure published (≈5.2×/4 threads on 18 6×6 openings,
  134,635 s → 25,900 s; superlinear — unexplained, no 2-thread point;
  11.8×/16 for SPDFPN per Čížek's citation). But the concurrency
  mechanism is not in this paper (SPDFPN 2014, unmined), no
  search-work inflation is reported, and sharing a mutable TT makes
  the journal-GHI contract bind at every TT site (L–XL; the inert TT
  refactor first — lean plan8 shape).
- **Surviving recommendation for plan 2 (input, not pre-decision):**
  hybrid — **C first** (harness over unmodified solver processes, S–M
  effort, constraints 1/2 vacuous at the worker level), **A behind it**
  as opt-in `--threads N` only after mining Pawlewicz & Hayward 2014,
  **B only as a cheap adjunct**, **D dead as a distinct option**
  (reduces to C + an idle-below-128-cores A).

## Discrepancies between the papers' assumptions and ours

- **All three domains are position-monotone with no repetitions and
  (for Sprouts/Breakthrough/Hex) no draws** — the parallel repetition
  question (constraint 2) is *unaddressed by the entire literature
  round*; that silence is the finding, and the journal contract stays
  load-bearing.
- **Scaling regime**: headline numbers live at 64–1024 cores on
  clusters; our consumers run 4–16 cores. All three extractions
  therefore carry explicit small-pool extrapolations, not quoted
  headlines.
- **Work accounting**: only Čížek reports a search-overhead ratio
  (node expansions) and only at ≥1 core aggregate; PPN₂ reports node
  counts (≈2× inflation at 64 clients); Solrex reports none. Our
  `child_evals` secondary metric has no direct published counterpart
  at 4–16 cores — the plan-2 spike must measure it.
- **Memory**: Čížek's workers get 5·10⁷-entry TTs each; PPN₂ bounds
  client trees by job size; our "RAM = TT only" contract must be
  restated as N×TT for any option (constraint 4).
- **Job granularity**: Sprouts ≈100 node expansions per Grundy number
  (slow expansion); Breakthrough jobs are 10³–10⁵ PNS descents; our
  jobs (solve a root child) are 10⁶⁺ child evals on m22/shuffle-win —
  much coarser, which helps utilization but worsens tail waste.
- Minor: the "333×" bibliography headline refines to 332.97× (with
  domain heuristic) / 208.67× (without); noted in the bibliography
  update.

## Backlog changes

- `parallel` #1: **mined**, pointers to the three research files
  (+ vendored PDFs), one-line finding per paper.
- `parallel` #2: enriched with the synthesis recommendation and the
  ordered spike measurements.
- `docs/bibliography.md`: three entries Open → Mined with pointers.
- `docs/plans/README.md` untouched (initiative neither opened, pivoted,
  nor closed this plan).

## Unresolved questions the plan-2 spike must answer

1. **Tail/utilization for C on our workloads**: PPN₂ never reports
   worker utilization; lean report7 measured the OR-side early exit as
   the dominant sequential saving. What fraction of wall time at 4–16
   workers is idle-tail on m22/shuffle-win, and does AND-child job
   granularity absorb it?
2. **`child_evals` inflation for C**: is the aggregate work inflation
   ≤ the published ≈1.0–2× band, or does master re-direction inflate
   more on our tree shape?
3. **Decisive-composition agreement**: does an isolated worker solve of
   each root child reproduce the sequential solver's child outcome
   (extends the two-solves-in-one-process property test to
   cross-process composition)?
4. **N×TT RAM**: what is the practical N before the multi-TT cost
   violates the documented resource envelope (constraint 4)?
5. **If A advances**: SPDFPN 2014 must be mined first (mechanism:
   sharding/locking/focussed children), and the plan10-hazard /
   first-player-loss-serialization questions re-examined for a shared
   mutable TT (journal §5's parallel-search constraint).

## Problems encountered

- No paywalled access issues except JLPNS: ResearchGate, CiteSeerX,
  author pages all unreachable; HAL is behind an anti-bot wall for
  direct fetches — solved via the Wayback Machine (`web.archive.org`)
  on the HAL document URL. Recorded in `research_jlpns.md` §0.
- pypdf required `fontTools` warnings on the Čížek PDF (CM fonts); the
  extracted text was complete and no glyph decoding issues affected
  any figure quoted (unlike the ghi_journal case).

## Missing tests / gaps

- None applicable (no code). The only test-adjacent gap is conceptual:
  the decisive-composition property test (unresolved question 3) does
  not exist yet and is a plan-2 deliverable.

## Next steps

1. Plan 2 (architecture note, `parallel` #2): weigh A–D per the
   synthesis; run the sizing spike in the recorded order; decide
   whether an opt-in parallel mode ships at all (lean trigger 1).
2. If A advances: mining round for Pawlewicz & Hayward 2014
   (*Scalable Parallel DFPN Search*, CG 2013, LNCS 8427) — add a
   bibliography entry before the plan.
3. Do not draft plan 3 (staged implementation) until #2 is GO.
