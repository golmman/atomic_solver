# Plan 5: Research reading round — Kishimoto & Müller journal GHI (2005)

Initiative: `conversion` backlog #5d. This is a **docs-only plan**: no
production code, no search-behavior change, no drift protocol. The
deliverables are one literature extraction (`research_ghi_journal.md`), a
soundness-contract sketch for `dfpn` backlog #4 (bounded cross-path
verification), and a re-ranked backlog. The plan ends at the contract —
the `dfpn` #4 implementation plan itself is *not* drafted here; if the
contract earns a go, it is opened in the `dfpn` initiative's numbering by
its owner.

This is the mining both papers from plan3 pointed at: EWS's
repetition-dominated 5×5 Go win used *caching-everything +
simulation-verified reuse* — the Kishimoto–Müller scheme
(`conversion/research_ews.md` §2) — and MOPNS defers cyclic graphs to the
same journal paper (`conversion/research_mopns.md` §4), citing the journal
version, not the AAAI-04 one.

Prerequisite reading: `dfpn/research_ghi.md` (the AAAI-04 mining, including
§9's honest list of what our current `simulate` does *not* do — it does not
carry the twin's original ancestor set, and on simulation failure it falls
back to base bounds instead of a bounded fresh search),
`dfpn/research_repetition_cache.md` (the repetition-dominated work profile:
96% re-proofs, cost in re-descent churn), `dfpn/report10.md` (the
threshold-folding hazard any reuse mechanism must structurally avoid),
`conversion/initiative.md` (working agreements, esp. #5 monotonicity-lemma
requirement), `conversion/report1.md` and `conversion/report2.md`
(the plan1 thin-surface and plan2 verification-lever no-gos, which this
plan must not re-litigate but must distinguish itself from).

## Goal

Answer, from the primary source, one question per aspect:

1. **The complete algorithm.** The AAAI-04 paper (PDF in repo:
   `docs/theory/ghi-2004/ghi-2004.pdf`) is abbreviated; the journal version contains
   the full algorithm. Extract the exact base/twin entry structure, path
   encoding, twin-creation rule, and **the simulation procedure in full** —
   in particular whether and how simulation carries or reconstructs the
   proof's *ancestor context* (repetition keys, move sequence). This is the
   concrete gap `dfpn/research_ghi.md` §9 documents in our implementation:
   our `simulate` seeds its path set with the current prefix but never
   carries the twin's original ancestor set, and cross-path reuse is
   therefore either accepted on incomplete evidence or rejected to base
   bounds.
2. **The soundness theorem.** Give the journal version's correctness
   statement (the AAAI-04 paper's Theorem 1 analog), its preconditions, and
   what exactly it guarantees (all returned proofs/disproofs correct under
   both first-player-loss and current-player-loss rules). Identify which
   preconditions our solver already meets (path-independent TT base
   entries, repetition-dependent draws never cached as solved — the
   first-player-loss shortcut) and which a bounded-verification lever would
   still need.
3. **The first-player-loss specialization.** Our repetition semantics are
   *repetition-as-draw* (two-fold-as-draw, `research_repetition_cache.md`
   §3), a third point between the paper's two rule variants. Map the
   paper's first-player-loss treatment onto repetition-as-draw: what
   changes in the twin/simulation argument when the repeated position
   evaluates as a draw rather than a loss for the repeating side, and does
   the paper's proof structure extend (the `dfpn` #4 backlog assumes it
   does — check, don't assume).
4. **Reuse-site economics and the plan10 hazard.** The journal algorithm
   resolves twins at *node entry* (lookup before expansion), not as child
   bounds. Our plan10 spike measured exactly this asymmetry: site-1
   (frame-entry) adoption was harmless but nearly empty; site-2
   (`evaluate_child`, folding `(0, INF)`/`(INF, 0)` into an unsolved
   parent's bounds) delivered the win *and* the destabilization. Extract
   where the journal algorithm returns reused results and how it updates
   the base entry on twin store (the `(1, 1)` re-initialization), then
   state explicitly whether the journal mechanism's reuse path can ever
   fold solved mass into unsolved threshold arithmetic — or why the
   plan10 failure mode (`dfpn/report10.md`: root-dn explosion on
   `m22_white`) does not apply.
5. **The soundness contract for `dfpn` #4.** The deliverable the next
   plan consumes: a sketched reuse rule in the shape of `conversion`
   backlog #1's lemma template (value claim, reuse direction, preconditions,
   worst-case defect class), the verification option the paper supports
   (simulation vs the backlog's "bounded fresh `dfpn` call at
   `max_depth = entry.depth`" — Option A of `dfpn/research_ghi.md` §9),
   an expected hit-surface estimate (plan1 measured the *unverified*
   cross-context draw surface as thin — does verification, which can also
   cover Win/Loss facts, change the economics, given plan10 showed
   Win/Loss reuse is only safe at frame entry?), and the mandatory gates
   (repetition soundness gate `tests/test_repetition.rs
   --include-ignored`; `m22_white` control; deterministic budget contract;
   quick-suite drift protocol).

The initiative's structural constraint is normative: the contract must
change **what is cacheable** (verified cross-path reuse widens the
cacheable surface beyond the plan9 exact-context cache) without touching
threshold arithmetic — or state why the plan10 failure mode does not apply
to its specific reuse path.

## Extraction format

Follow the house style of `dfpn/research_*.md` (`dfpn/research_ghi.md` is
the direct predecessor — this extraction extends, and where the journal
version supersedes it, *corrects*, that file; mark such points
explicitly): standalone numbered sections, algorithm descriptions in the
paper's terms, every paper claim cited to the paper's section/theorem,
every solver claim cited to file/function. Do not copy the paper
wholesale — extract the machinery, the soundness conditions, and the
empirical results relevant to bounded cross-path reuse in a
repetition-dominated regime.

## Placement decision

The extraction file goes to **`docs/plans/dfpn/research_ghi_journal.md`**,
not `conversion/`: its consumer is `dfpn` backlog #4, it is a direct
continuation of `dfpn/research_ghi.md` (same topic, same terminology, §9
gaps to resolve), and the `dfpn` initiative will own the follow-up
implementation plan. `conversion` backlog #5d and `docs/bibliography.md`
link to it. This deviates from plan3's keep-everything-in-`conversion`
pattern deliberately; the rationale is recorded here so the next reader
does not "fix" it back.

## Sources

- Primary: A. Kishimoto, M. Müller, *A Solution to the GHI Problem for
  Depth-First Proof-Number Search*, Information Sciences 175(4), pp.
  296–314 (2005). Paywalled on ScienceDirect; locate an open/author copy
  first (Kishimoto's Alberta author pages, university repositories,
  Semantic Scholar/OpenAlex `open_access` locations — the MOPNS and plan3
  rounds both resolved copies this way). Record the exact version read
  (journal PDF vs author manuscript) in the research file and report.
- Cross-check: the AAAI-04 version, `docs/theory/ghi-2004/ghi-2004.pdf` (in repo),
  already mined in `dfpn/research_ghi.md` — use it to identify precisely
  what the journal version adds (expected: complete proofs, the full
  simulation procedure, df-pn-specific threshold details), not to
  substitute for it.
- Terminology reuse: `dfpn/research_ghi.md` (base/twin, simulation,
  first-player-loss); do not redefine.

## Research extraction targets (`research_ghi_journal.md`)

1. **Complete algorithm** (§ Goal 1): entry structure, path codes, twin
   lifecycle (create/simulate/adopt/reject), base re-initialization, root
   threshold handling; a step-by-step simulation procedure precise enough
   to implement against.
2. **Soundness theorem** (§ Goal 2): statement, preconditions, proof
   obligations; which preconditions our TT discipline already satisfies
   (cite `src/search/tt/`, the first-player-loss shortcut in
   `src/search/dfpn/core.rs::dfpn` store site, the plan9 repetition cache
   contract in `src/search/dfpn/repetition_cache.rs`).
3. **Repetition-as-draw mapping** (§ Goal 3): where the paper's argument
   uses "repetition = loss for the repeating side" and what replaces it
   under our semantics; note that our repetition edges can never support a
   decisive proof (`research_repetition_cache.md` §4 point 1) and whether
   that simplification is preserved or needed by the paper's proof.
4. **Reuse-site analysis** (§ Goal 4): journal lookup/return order vs our
   `dfpn` node entry and `evaluate_child` probe sites; plan10-hazard
   verdict with the mechanism-level reason, referencing the report10
   measurements.
5. **Soundness contract sketch** (§ Goal 5): the lemma-shaped reuse rule,
   verification-option comparison (paper's simulation vs bounded fresh
   search: cost, completeness, implementation surface in
   `src/search/dfpn/`), hit-surface estimate, gates, and open risks.
6. **Mapping section** ending in the summary table row (mechanism, where
   it lives in our code, what is cacheable / line source / thresholds,
   plan10-hazard verdict, effort S/M/L, go/no-go for opening the `dfpn`
   #4 implementation plan).

## Tasks

1. Locate and read the journal paper (recording the version actually
   read); write `docs/plans/dfpn/research_ghi_journal.md` with the
   sections above.
2. Update `dfpn/initiative.md` backlog #4: link the new extraction, note
   whether the contract clears the plan10 hazard and what the backlog's
   "do not attempt before #1's spike lands" dependency resolves to now
   (#1 closed no-go in `conversion`, lemma retained as the candidate
   argument — the contract either adopts it or replaces it).
3. Update `conversion/initiative.md`: backlog #5d status (mined, closed;
   (c)/(e) remain open), History entry.
4. Update `docs/bibliography.md`: journal GHI entry **Open → Mined** with
   the pointer to `dfpn/research_ghi_journal.md` (keep the `dfpn` #4
   backlink — the lever stays open there even if the contract says no-go).
5. Sanity-check the gate: `cargo fmt --check` / `cargo test --release`
   must be green (nothing in `src/` may have changed; `git status` must
   show docs only), and the new file must not dangle references into
   `docs/spec/` rules (plan-side docs may cross-reference `docs/plans/`
   internals).
6. Write `report5.md` in this directory: version(s) actually read and how
   the copy was located, discrepancies vs `dfpn/research_ghi.md`'s AAAI-04
   reading and vs our assumptions, the contract verdict (go/no-go for the
   `dfpn` #4 implementation plan), backlogs opened/closed, unresolved
   questions, next steps.

## Non-goals

- Any production code change, benchmark run, or drift protocol (there is
  nothing to drift). The plan's output is the contract, not a lever.
- Drafting the `dfpn` #4 implementation plan — that is the next plan, in
  the `dfpn` initiative, owned there. This round ends at the re-ranked
  backlog and the contract sketch.
- Mining backlog #5c (Čížek 2025 parallel PNS, feeds `conversion` #4 /
  `lean` #2) or #5e (Gao complexity) — separate owning backlogs, do not
  duplicate.
- Re-litigating closed no-gos: plan1 (unverified superset-context draw
  cache — thin surface), plan2/2b (engine candidate-line verification —
  cooperative-horizon lines; note explicitly that `dfpn` #4 verifies
  *solver-proven twins with complete proof trees*, a different object, but
  do not borrow its optimism), plan10 (Win/Loss reuse as child bounds —
  the hazard this plan's contract must route around, not repeat). Each may
  be revisited only at the mechanism level: the journal paper's machinery
  must be shown to structurally avoid the measured failure modes, or the
  verdict is no-go.
- Changing the first-player-loss shortcut, the plan9 repetition cache
  semantics, or TT path-independence in this round — those are inputs the
  contract must respect, not surfaces it may redesign.
