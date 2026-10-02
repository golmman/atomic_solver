# Plan 3 Report — Mine SPDFPN 2014, option-A reassessment

Executed 2026-10-02 per `plan3.md`. **Docs-only plan confirmed**: no
`src/` or `examples/` change, no benchmark runs, no drift protocol.

## Paper actually read (version, provenance)

| Paper | Version read | Pages | Vendored |
|---|---|---|---|
| Pawlewicz & Hayward 2014 (*Scalable Parallel DFPN Search*, CG 2013, LNCS 8427, pp. 138–150) | **Author copy** from Hayward's site (`webdocs.cs.ualberta.ca/~hayward/papers/pawlhayw.pdf`); Springer paywalled | 13 | `docs/theory/spdfpn-2014/spdfpn-2014.pdf` |

**Bibliography correction (finding)**: the entry's `arXiv:1503.07698`
pointer is wrong — that id resolves to an unrelated XENON1T
dark-matter-detector paper. No arXiv version of SPDFPN exists; the entry
now points at the DOI and the vendored author copy.

## Deliverables

- `docs/theory/spdfpn-2014/` — PDF + full-text extraction (provenance
  header records the wrong-arXiv history).
- `research_spdfpn.md` — the mining analysis: full mechanism
  (W-threshold interruptible jobs + same-child resume clause; pn-guided
  `TRYRUNJOB` assignment on the TT's past-work field; virtual win/loss +
  per-ply virtual TT + job lock; shared-TT locking/overwrite/least-work
  replacement), measured efficiency (0.74/16 threads, f_t ≤ 1.22,
  **work inflation ≤ 1.40×** — vs option C's 15.5×), the
  repetitions-under-concurrency silence and why our path-independent TT
  payload + thread-local repetition cache covers it, the component
  mapping/bill of materials, and the staged plan skeleton.
- `parallel/initiative.md` — reopened (premise change in Status, backlog
  #4 opened, History entry).
- `docs/bibliography.md` — corrected + **Open → Mined**;
  `docs/theory/README.md` — new row.
- `docs/plans/README.md` — `parallel` row updated (reopen event).

## Verdict

**GO input for the A-stage** (decision for the owner): SPDFPN is the
strongest published option-A shape and the only remaining multiplicative
lever (`structural_floor.md` §9); its nondeterminism envelope (which
valid proof wins, never a false one) matches the newly accepted premise;
its serial base is our own serial base (dfpn 1+ε). Recommended staging:
plan4 = inert TT-concurrency refactor (drift-gated, the cheap kill
point), plan5 = prototype (`--threads N`, W-threshold + virtual TT +
TRYRUNJOB) measured on the hard class with pre-registered GO bands.
Domain-transfer risk flagged: Hex credits its scaling partly to
VC-engine-dominated (expensive) nodes; our movegen/eval nodes are cheap,
so efficiency below the paper's 0.74/16 is plausible — the prototype
must measure, not assume.

## Additional tools used

- `pypdf` (pip-installed) for text extraction, per theory-library
  convention; DuckDuckGo HTML search to locate the author-copy URL after
  the arXiv pointer proved wrong.

## Problems encountered

- Bibliography arXiv pointer wrong (see above) — fixed rather than
  propagated.
- First download attempt returned the XENON1T PDF (same wrong id); the
  page count and content check caught it before vendoring analysis.

## Unresolved / next steps

- plan4 (TT-concurrency refactor) and plan5 (prototype + measurement)
  not yet written; open them only after the owner confirms the A-stage
  GO. If plan4's drift gate fails, the A-stage dies there.
- The ProofEvent ordering contract for a threaded solver is designed but
  undecided (prototype: serialize emissions behind a mutex; revisit when
  proof-tree integration matters).

SESSION COMPLETE
- plan3.md + report3.md written; extraction vendored (`docs/theory/spdfpn-2014/`);
  `research_spdfpn.md` written; bibliography corrected + flipped to Mined;
  theory README + plans README rows updated; `parallel` initiative reopened
  (backlog #4). Gate: n/a (docs-only).
Follow-up options:
  1. **"Execute parallel plan4"** — the inert TT-concurrency refactor
     (sharded TT, interior-mut locking, byte-identical N=1 behavior):
     the drift-gated prerequisite and cheap kill point for the A-stage.
  2. Defer: keep `parallel` reopened but dormant until a consumer
     commit to the hard-class solve workload exists; the mining stays
     valid indefinitely.
