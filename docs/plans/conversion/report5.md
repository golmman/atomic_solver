# Report 5 — Journal GHI reading round (Kishimoto & Müller 2005)

**Verdict: docs-only round complete. `dfpn` #4 closed as an evidence-based
no-go.** The journal paper was located, read in full, and mined into
`docs/plans/dfpn/research_ghi_journal.md` (author copy vendored alongside as
`docs/theory/ghi-journal-2005/ghi-journal-2005.pdf`). The soundness contract for bounded cross-path
verification was written as planned — and the evidence says the lever is
economically empty on our position class, so the contract closes `dfpn` #4
instead of opening an implementation plan. No production code changed;
`git status` shows the two initiative files, the bibliography, and two new
docs files only.

## Version(s) actually read and how the copy was located

- **Primary, read in full**: the journal version — A. Kishimoto, M. Müller,
  *A solution to the GHI problem for depth-first proof-number search*,
  Information Sciences 175(4), pp. 296–314 (2005), DOI
  `10.1016/j.ins.2004.04.012`. 19 pages, publisher pagination, accepted
  26 April 2004.
- **Location path**: OpenAlex reports the work closed (`oa_status: closed`,
  no OA URL); Semantic Scholar and Unpaywall agree. The OpenAlex `locations`
  list, however, preserves a CiteSeerX record
  (`oai:CiteSeerX.psu:10.1.1.69.818`) whose raw source URL is the author
  copy on Müller's Alberta page —
  `http://www.cs.ualberta.ca/~mmueller/ps/kishimoto-mueller-infsci-ghi.pdf`
  — which still serves the PDF (254,275 bytes, application/pdf). Same
  resolution pattern as plan3 (canonical index → location records → author
  copy). The copy is vendored as `docs/theory/ghi-journal-2005/ghi-journal-2005.pdf` (same
  precedent as `docs/theory/ghi-2004/ghi-2004.pdf`, `docs/theory/pdfpn-2010/pdfpn-2010.pdf`, `docs/theory/epsilon-trick-2007/epsilon-trick-2007.pdf`) so the round is
  reproducible if the URL rots.
- **Cross-check**: the AAAI-04 version (`docs/theory/ghi-2004/ghi-2004.pdf`), read
  side-by-side for the delta, and the already-mined `dfpn/research_ghi.md`.

## What the journal version actually adds (vs plan5's expectations)

plan5 expected "complete proofs, the full simulation procedure,
df-pn-specific threshold details". Measured against the text: **2 of 3**.

- **Added**: the complete literature review (§2.2 — Palay, Campbell's
  draw-first/draw-last, Breuker's BTA with three documented deficiencies,
  Nagai's approach with two drawbacks, Schijf's tree/DAG/DCG methods); the
  df-pn pseudo-code (Fig. 2, adapted from Nagai); the soundness argument as
  **Theorems 3.1 and 3.2 with proofs** (AAAI-04 states Theorem 1 unproven and
  explicitly defers the df-pn proof to the journal); the controlled
  DUP/+SIM/+NOCYCLE ablation (Table 1); Appendix A (why BTA must clear
  possible-draw marks); two efficiency sentences on simulation.
- **Not added**: a step-by-step simulation procedure. §3.3 is at the same
  abstraction level as the AAAI-04 section; neither version contains
  pseudo-code for the twin/simulation hooks. In particular, **neither
  version carries or reconstructs the twin's original ancestor set**, and
  the journal's specified failure handling is exactly the one our plan5/7
  port had (fall back to base bounds; no bounded re-search). The concrete
  gap `dfpn/research_ghi.md` §9 documents for our implementation is
  therefore a gap in the primary source's *specification*, not a defect of
  our port — the single most decision-relevant finding of this round. The
  remaining published source at the implementation level is Kishimoto's
  2005 Ph.D. dissertation (open access on the UAlberta repository;
  bibliography entry stays **Cited**) — flagged as next-in-line if the
  parallel spike ever needs the full procedure, not mined here.

## Discrepancies vs `research_ghi.md` and vs our assumptions

1. **Root thresholds (correction recorded in the extraction, §1.4).**
   `research_ghi.md` §4.2 states the modified scheme initializes root
   thresholds to `(1, 1)` and the original to "one large value" — inverted.
   Both published versions say the modified scheme initializes them to the
   large value **∞ − 1** (journal §3.5: "Following Nagai's approach, we
   initialize both thresholds at the root to ∞ − 1"; AAAI-04: "to ∞ − 1,
   not ∞ as in the original df-pn algorithm"). Extraction caveat: the
   journal PDF's symbol font has a broken ToUnicode map (∞ extracts as
   `1`, minus as an unmapped glyph); the sentences were decoded from glyph
   outlines and confirmed against the AAAI text's clean `∞ − 1`. The two
   versions genuinely disagree only about the *original* df-pn's root
   initialization ("not 1" per journal §2.2.4 vs "not ∞" per AAAI-04) —
   Nagai's thesis was not consulted; noted as an open bibliographic point.
   Practical impact on us: none (our `dfpn` stores at frame exit, never
   thresholds before expansion), but the correction prevents a future
   reader from "fixing" a design based on the inverted note.
2. **The theorems' scope is narrower than our assumption.** The journal
   theorems assume a **sound verification oracle** ("a search is performed
   below Y and X in our algorithm") — they prove the twin store/lookup
   discipline routes around draw-first/draw-last; they do not prove
   simulation sound. The plan7 false win entered exactly through that
   assumed step. Any reuse lever in this codebase must supply its own
   soundness argument (the plan1 monotonicity lemma or a self-sound bounded
   re-proof); the paper cannot carry it.
3. **The repetition-as-draw extension holds only vacuously.** `dfpn` #4's
   premise "the paper's proof structure extends to our semantics" was
   checked, not assumed: under two-fold-repetition-as-draw, decisive facts
   are path-independent *by rule* (repetition edges evaluate `Draw`;
   `research_repetition_cache.md` §4 point 1) — strictly stronger than the
   paper's first-player-loss scenario, where disproofs are still
   path-dependent and need twins. The twin machinery is therefore never
   needed for decisive facts here, and the paper has no machinery at all
   for our actual path-dependent object (draw proofs). The BTA draw-as-
   disproof reduction (§2.2.3) is explicitly unavailable to us.
4. **The reuse-site reading confirmed plan5's asymmetry hypothesis.** The
   journal resolves twins at node entry (base → exact-path twin →
   simulation → adopt-or-fallback, §3.1/§3.4), never as child bounds from a
   parent-level probe — and the (1, 1) base re-initialization has a second,
   previously unrecorded effect: it keeps a twin's solved numbers invisible
   to the parent's df-pn threshold computation until node entry. Plan10's
   failure mode (cheap unverified site-2 adoption flooding threshold
   arithmetic; `dfpn/report10.md`) has no analog in the journal mechanism.
5. **Table 1 datapoint worth keeping**: NOCYCLE cuts simulation calls from
   239,820 to 356 over 134 problems (~674×) and makes the correct solver as
   cheap as the GHI-ignoring one (82 s vs 81 s, 156 flawed TT entries
   caught). The overhead question is decided by how few path-dependent
   results exist — and our design has already driven that count to zero for
   decisive facts.

## The contract and its verdict

The contract (`research_ghi_journal.md` §5, mapping row §6) adopts plan1's
monotonicity lemma as the value claim, fixes the reuse direction to
frame-entry with TT store-back (completing the propagation channel the
paper leaves unspecified — mechanically plan11 arm A's shape), rules the
paper's simulation unavailable as machinery (our TT stores no proof trees;
its soundness is assumed anyway), and lists the mandatory gates
(repetition soundness gate, `m22_white` control, quick-suite drift,
deterministic budget contract, two-solves-in-one-process property test).

**No-go**, on two independent measured bounders:

- **Win/Loss side**: the journal mechanism (frame-entry + per-reuse
  verification) is a strict subset of plan11 arm A (frame-entry, unverified
  — no simulation needed since decisive facts are path-independent by rule)
  at strictly higher per-hit cost. Arm A measured −8.8% stress
  first-outcome / −2.6% default (`dfpn/report11.md`) — below the 10% gate,
  useless in default mode. Verification cannot widen the surface, so it
  cannot change the economics (plan5's explicit question: answered no).
- **Draw side**: the re-proofs a cross-path draw cache would save cost
  ≈ 1.1 child evals each (`research_repetition_cache.md` §2:
  34,118 `repeat_proof_work` over 31,620 re-proofs); any verification-based
  reuse costs at least what it saves, and plan9's zero-verification
  exact-context cache already intercepts ~97% of the widening surface
  (`conversion/report1.md`).

The structural soundness of the journal mechanism (node-entry return,
verification-bounded volume, (1, 1) re-init) is what forfeits the plan10
win — the benefit and the pathology were one mechanism, and the journal
mechanism keeps the safety. This is a *consistent* negative: it closes the
loop opened by report10 rather than contradicting it.

## Backlogs opened/closed

- **Closed**: `dfpn` backlog #4 (bounded cross-path verification) —
  evidence-based no-go, contract retained; `conversion` backlog #5 item (d)
  — mined. Bibliography journal-GHI entry **Open → Mined** (lever pointer
  kept, redirected to the parallel-spike constraint).
- **Opened**: none.
- **Re-ranked**: the contract's reuse rule is recorded in `dfpn/initiative.md`
  #4 and both initiatives' History as a **design constraint for the
  parallel-search spike** (`conversion` #4 / `lean` #2): with shared-TT
  multi-worker search, cross-worker path contexts differ by construction,
  the plan9 exact-context cache degenerates, and the journal mechanism is
  the published reference for keeping cross-worker reuse sound. There,
  cross-path reuse is not an optimization over cheap re-proofs — it is the
  difference between parallel and serialized search, the only lever of
  2–8× size left.

## Problems encountered

1. **Broken ToUnicode in the journal PDF's symbol font** (see discrepancy 1):
   naive text extraction silently corrupts `∞`/`−` in §2.2.4 and §3.5 — the
   two most safety-relevant threshold statements in the paper. Decoded via
   glyph outlines (`fontTools` CFF charstring bounds: ∞ = wide baseline
   glyph, minus = 42-unit bar) and cross-checked against the AAAI text.
   Anyone re-reading this PDF should not trust raw copy-paste of those
   sentences.
2. OpenAlex/Semantic Scholar both index the journal as closed with no OA
   tag; the author copy was only reachable through the CiteSeerX location
   record's raw source URL (the CiteSeerX document endpoint itself 404s).
   Vendoring the PDF was the robust move.
3. `research_ghi.md` §4.2's inverted root-threshold note survived two
   mining rounds (plan5-of-dfpn and the §9 review). Nothing in this round
   depends on it, but it is a reminder that AAAI/Elsevier symbol-font PDFs
   have burned this repo's extractions before — worth remembering when the
   dissertation is mined.

## Unresolved questions

- The original df-pn's root-threshold initialization (journal: 1; AAAI: ∞)
  — resolvable only from Nagai's thesis; no design decision here depends on
  it.
- The propagation channel for a *verified* reused (dis)proof to the parent
  in the journal mechanism (§1.4 of the extraction): the paper re-inits the
  base to (1, 1) on twin store and saves verified reuses in twins, which
  would hide the proof from the parent's threshold computation. Our
  architecture resolves it naturally (store-back as a normal solved entry
  = arm A), but the paper's own implementation detail is unpublished —
  likely in the dissertation.
- The NOCYCLE condition ("(dis)proven without detecting a repetition") is
  never defined precisely; our `repetition_seen` chain is one sound
  operationalization. If the parallel spike adopts the journal mechanism,
  the exact definition matters for twin volume and should come from the
  dissertation.

## Sanity checks

- `git status --porcelain`: two new docs files
  (`dfpn/research_ghi_journal.md`, `docs/theory/ghi-journal-2005/ghi-journal-2005.pdf`) + three modified
  docs files (`docs/bibliography.md`,
  `docs/plans/conversion/initiative.md`, `docs/plans/dfpn/initiative.md`).
  Nothing under `src/`, `tests/`, `examples/`.
- `cargo fmt --check`: clean.
- `CARGO_PROFILE_RELEASE_LTO=thin cargo test --release`: all binaries green
  (218 unit + fast integration, 0 failed; ignored = the slow tier, as
  designed).
- The new extraction file contains no `docs/spec/` references (plan-side
  docs may cross-reference `docs/plans/` internals; it does).

## Next steps

- The parallel-search design spike (`conversion` #4 / `lean` #2) inherits
  the contract as a hard constraint: shared-TT worker reuse must follow the
  journal discipline (decisive facts monotone/context-free; draw facts
  exact-context or verified; no child-bound injection). If that spike is
  opened next, mining Kishimoto's dissertation first would be the
  cheap prerequisite (open access; contains the implementation-level
  simulation detail this round could not find in the journal paper).
- `conversion` backlog #5 items (c) (Čížek 2025 parallel PNS) and (e) (Gao
  complexity) remain open with their owning backlogs — (c) is the natural
  companion reading before the parallel spike opens.
- The remaining sequential-search levers for the stress class are `dfpn` #3
  (refinement after cap-cut) and parallelism; no reuse-shaped lever remains
  open after this round.
