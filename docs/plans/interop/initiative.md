# Initiative: `interop` — external proof validation and interchange

## Status

**Active (opened 2026-10-29).** The first plan is
[`plan1.md`](plan1.md): a differential proof-checking oracle against
[`aiorla/provari`](https://github.com/aiorla/provari), using provari as an
external executable and never as linked code. Next plan number if the
initiative continues: **plan2**.

The initiative was opened after a read-only review of provari (commit
`48686bc6c786d6ea70568f0b2ad4002cf0520206`, `v0.1.0`). Provari is an
independent Atomic prover/checker written against `shakmaty`; its checker
shares no code with its search and regenerates every defence from an
attacker-only proof. It is useful precisely where our current validation is
weakest: our proof-tree validator and `proofdb_verify` replay proofs with the
same `atomic-movegen`-based `Position` implementation the search used.

## Goal

Make `atomic_solver` proof artifacts **independently checkable by a second
implementation**, and — where it is cheap and does not compromise the proof DB
as the source of truth — **exchangeable** in portable, attacker-only forms.

Priorities follow `AGENTS.md`: correctness first, then maintainability,
performance, and memory. This initiative is deliberately about evidence and
interchange, not about changing the search.

## Non-goals

- **No GPL code import and no product dependency on provari.** Provari is
  GPL-3.0-or-later; this repository is MIT. The oracle runs as a separate
  process, its source and binary are not vendored, and only drivers/results
  are committed.
- **No change to search semantics or proof-tree construction.** A plan that
  finds a solver/proof-tree defect reports it; fixing it belongs to the owning
  initiative (`dfpn`, `proof`, or `proofdb`).
- **No replacement of the proof DB or its external schema.** The SQLite
  `nodes`/`shards` model stays the source of truth. Compact proof files are
  derived artifacts.
- **No NNUE/tablebase integration.** Atomic Syzygy tables are noted as an
  oracle opportunity in `egtb`; they are not wired into this initiative.
- **No PPV productization or proof-tree dump-format replacement.**

## Backlog

| # | Item | Mechanism | Why | Effort | Status |
|---|------|-----------|-----|--------|--------|
| 1 | Differential proof checker oracle | Export an existing `proof_tree.bin` to provari's JSONL format; run `provari check`; classify every mismatch | Independent rules implementation catches correlated `atomic-movegen`/`Position` bugs; the exporter is reusable in later campaigns | M | **planned — `plan1.md`** |
| 2 | Root-`Loss` coverage and compact proof export | JSONL only supports attacker = side-to-move, so root-`Loss` trees need the compact `.pvr` format (`attacker` header) or per-child decomposition | Completes the oracle's coverage; produces a genuinely compact certificate | M | open (gated by plan1's exporter/triage results) |
| 3 | Portable attacker-only export for proofdb transport | Derive an attacker-only shard/export from the DB tree; merger or a checker regenerates defence nodes | Shrinks shards/transport and gives external consumers a spec-sized proof; the DB keeps full rows | M–L | open (only if plan1/2 shows the format is stable) |
| 4 | External reference corpus | Run the solver on provari's `bench/high200.jsonl` (or a subset) as an out-of-distribution benchmark | An external, adversarial corpus catches overfitting to our own suites | S–M | open |
| 5 | Native independent checker | A second, small rules implementation owned by this repo, sharing no search/movegen code | Removes the external-binary and GPL constraints from the routine gate | L | open (design only; do not start without a consumer) |

An item stays open until a plan claims it. Plans are single-lever and sized
for one session; a plan whose only product is evidence still ends with a
report.

## Conventions

- **Oracle isolation.** Build provari in a scratch directory (`/tmp` or an
  ignored tools directory), pin the commit/tag, and record
  `provari --version`. Do not add it to `Cargo.toml` or the workspace.
- **The export is derived, never authoritative.** The proof tree/DB remains
  the thing validated; the external checker is a second opinion, not a new
  source of truth.
- **Every non-accept is classified.** A provari `valid: false` is not
  automatically a defect: provari's no-repetition convention is stronger than
  our clock-inclusive cycle check. The classes are `accepted`,
  `convention-repeat`, `unchecked` (limits/tablebases), `export-bug`, and
  `suspect` (a possible solver/proof-tree defect). Only `suspect`/`export-bug`
  trigger follow-up work.
- **Record the exact sample.** The tree list, hashes/sizes, provari command,
  and per-tree verdicts go under `measurements/planN/`; no raw solver
  transcripts or GPL sources are committed.
- **No drift.** A plan that adds only examples/measurement tooling must leave
  search output bit-identical. `make test` is the default gate; `make
  test-full` is required only if a plan touches search/TT/proof construction.

## Decision record 2026-10-29 (initiative opened)

The provari review offered three broad levers: an independent checker /
compact proof format, DF-PN search ideas (move-class starting numbers,
quiet-move grouping, sibling-proof simulation, a learned provability model),
and public Atomic Syzygy tablebases.

This initiative takes the first lever because it is the cheapest and most
directly serves the top quality attribute (correctness). The DF-PN ideas are
search-semantics changes and belong to the search-performance side; as of
2026-10-29 they have a dedicated active home,
[`../provari/initiative.md`](../provari/initiative.md), with the pinned
measurement gate. The tablebase fact belongs to `egtb`, where the measured
≤6-men coverage (0.009% decisive, 0.0002% stress) keeps the faster-solves goal
closed and leaves an oracle opportunity. `interop` stays validation and
interchange only.

Plan1 is intentionally narrow: **root-`Win` trees only**, JSONL export, an
external `provari check`, and triage. The compact/root-`Loss` work is
backlog #2, not a hidden part of plan1.

## Measurement layout

`docs/plans/interop/measurements/planN/` holds:

- commit: the driver(s), the sample list, parsed results (`*.json`/`*.csv`),
  and the report's provenance table;
- never commit: the provari source clone/binary, raw `*.jsonl` exports for
  every tree (regenerable from the committed sample and exporter), solver
  transcripts, or `__pycache__`.

## History

- **2026-10-29** — initiative opened; plan1 drafted for the differential
  checker oracle. No product code changed in the opening session.