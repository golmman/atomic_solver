# Provari Plan 4: static immediate-explosion closure (item 4a, opt-in)

Initiative: `provari`, backlog item 4 (first half — split as 4a/4b). Drafted
2026-10-30. Status: ready for execution. This plan changes search behavior
behind a **default-off** flag; it does not change any correctness contract.

Prerequisite reading: [`initiative.md`](initiative.md), [`report2.md`](report2.md),
[`report3.md`](report3.md), `AGENTS.md`, and the node-entry/terminal/store
docs in `src/search/dfpn/{core,children}.rs`.

## Why this plan exists

Plan 2 (move-class starting numbers) and plan 3 (quiet-move grouping) reduced
the core FEN from 437M to 27.9M child evals and 60.1 s to 5.47 s with both
flags on. The next ranked provari lever is **static atomic tactical closure**
(backlog item 4): avoid expanding forced tactics such as an immediate
exploding capture, an exposed king, a no-safe-evasion check, a parry-less
double threat, a mate in one, or cannot-win material.

Item 4 is too large for one good session and its sub-mechanisms have different
risk profiles. This plan executes the **cheapest, sound primitive** first:
**a legal capture that explodes the opponent's commoner at once**. Provari's
`exploding_capture` (`src/moves.rs`, consulted read-only) is the shared
building block used by its `exposed`, check/mate, and simulator paths. Making
our solver recognize it at node entry is one mechanism, needs no new movegen
(the frame already has its legal move list), and its result is an ordinary
one-move win.

Deliberately out of scope (plan 4b / later):

- provari's `exposed` two-ply closure (every defender reply allows an
  immediate explosion), the no-safe-evasion check filter, and the parry-less
  double-threat static verdict;
- mate-in-one as a dedicated pre-scan (our child sweep already returns a
  terminal `Loss` for a mating child);
- cannot-win material draws, blast-aware exchange precision, EF-PN, learnable
  guides, and any change to the win-only front-end (items 2 and 5).

## Plan-session probe (read-only, not committed)

The provari clone from earlier plans still lives in `/tmp/provari`. Its
`src/moves.rs::exploding_capture` and the static paths around
`src/dfpn.rs`'s `exposed`/`recomputed` were re-read to confirm the target
shape; no provari code was imported, copied, or added. No repository file was
changed by the plan session.

Observation for planning: provari's static explosion check is a bitboard
target test over an already-known board (`king_attacks(enemy_commoner)`,
enemy pieces, minus squares beside our own commoner), not a search. Our node
entry already generates its legal move list before the TT probe, so the same
idea can be implemented as a **legal-move scan with no extra movegen**.

## Scope decision (one lever)

Implement **static immediate-explosion closure** at `dfpn` node entry behind a
default-off `--static-tactics` flag:

- if the side to move has a legal capture by a non-commoner of an enemy piece
  adjacent to the enemy commoner (and not adjacent to our own commoner), the
  node is a **Win in one move** — store it as a solved TT entry and return.

This is the atomic "capture beside the king" win. It is sound for both OR and
AND nodes because the result is from the side-to-move perspective and depends
only on the current position and its legal moves.

## Design

### 1. The primitive

New module `src/search/dfpn/static_tactics.rs` (target < 10 KB; tests may live
in `static_tactics/tests.rs`):

```rust
pub(super) fn immediate_exploding_capture(
    board: &Board,
    moves: &MoveList,
) -> Option<Move>;
```

It returns the first legal move from the frame's existing move list that is an
immediate explosion. It must not generate moves, play moves, mutate the
position, or touch the TT.

Target shape (provari's `exploding_capture`):

- `them = !board.side_to_move()`, `us = board.side_to_move()`;
- `targets = union(king_attacks(c) for c in board.commoners(them))`;
- `targets &= board.pieces_color(them) & !board.pieces_pt(Commoner)`;
- `targets &= !union(king_attacks(c) for c in board.commoners(us))` — a blast
  beside our own commoner would remove it too;
- for each `mv` in `moves` with `board.is_capture(mv)`, non-en-passant,
  moving piece not a commoner, and `mv.to_sq()` in `targets`: return `Some(mv)`.

If the enemy commoner is already gone the node is terminal and never reaches
this module. A direct capture of the enemy commoner is not part of the target
shape; it cannot occur in a legal game (the opponent would have had to leave
the commoner attacked on our turn), and our terminal child classification
already covers it if it does.

Documented approximations:

- **Only beside-commoner captures.** Provari's static closure also derives
  exposed-defender and mate-in-one verdicts; this plan does not.
- **Legal-move scan only.** The result is a move from `Position`'s legal move
  list, so no pseudo-legal candidate can create a false win. A true explosion
  the move generator rejects is not claimed.
- **No own-commoner blast.** Targets adjacent to our own commoner are
  excluded; that move would explode our commoner too / is not a safe win.
- **No miss is a correctness issue.** Missing a static win only leaves the
  position to the ordinary search; its result must be identical.

### 2. Node-entry hook

In `Search::dfpn` (`src/search/dfpn/core.rs`), after the `max_depth` leaf
check, the local repetition check, the TT solved-result check, and the
repetition-cache probe — but **before** the TT ordering hint / `sort_moves`
and the child sweep — add a flag-gated branch:

```text
if self.static_tactics
    && let Some(mv) = static_tactics::immediate_exploding_capture(pos.board(), moves)
{
    let outcome = Outcome::Win;
    let (pn, dn) = outcome.pn_dn_for(is_or_node);
    self.tt.store(tt_key, mv, u8::MAX, 0, Some(outcome), pn, dn, 1, u32::MAX);
    self.emit_proof_node(pos, outcome, 1);
    self.precompute_pool[slot_depth] = slot;
    return outcome;
}
```

Placement matters and is part of the contract:

- **After terminal detection** — terminal outcomes outrank everything.
- **After `max_depth == 0`** — the depth horizon is unchanged; a depth-0 leaf
  still returns Draw and stores the leaf entry.
- **After the local repetition check** — a repeated position still returns
  Draw exactly as shipped.
- **After the TT solved and repetition-cache fast paths** — an existing proof
  is still preferred; the static claim only preempts expansion.
- **Before sorting** — the frame's move list is already generated; no extra
  movegen is needed and the ordering hint is irrelevant to a solved node.

Store shape: `best_move = mv`, `best_child = u8::MAX`, `work = 0`,
`outcome = Win`, `depth = 1`, `remaining_depth = u32::MAX`. The proof event is
`NodeProven(path_to_node, node_hash, Win, 1)`. `work = 0` records that the
proof was free (no child evaluation), consistent with terminal stores.

### 3. Budget and counters

The target scan and attack bitboards are internal work and are **not** billed
as child evaluations. A static claim consumes no `child_evals`, no restart
slice, and no budget. The frame exits before any child evaluation, so a run
with the flag on may have strictly fewer `child_evals`; it must never have
more because of this hook.

### 4. Configuration and defaults

- `Search::static_tactics: bool`, default `false`, `src/search/dfpn/mod.rs`.
- `Search::set_static_tactics(bool)` in `src/search/dfpn/config.rs`.
- `CliOptions::static_tactics: bool`, default `false`, `src/cli.rs`; parser arm
  `--static-tactics`; one help entry in `src/main.rs::HELP` marked
  experimental/default off; wire in `main`.
- Extend the CLI help golden fixture and parser tests for default-off and
  presence.
- Document that this flag is independent of `--move-class-pn` and
  `--quiet-grouping`; measure all combinations in the protocol.

## Tasks

1. **Read and confirm.** Re-read this plan, `report3.md`, and the `dfpn`
   node-entry order (terminal → depth → repetition → TT → repetition cache →
   ordering → children). Confirm the scope: no exposed-defender closure, no
   mate-in-one pre-scan, no cannot-win material, no EF-PN.
2. **Add the primitive.** Create `src/search/dfpn/static_tactics.rs` with
   `immediate_exploding_capture`, the target bitboard helper, and the module
   header approximations. Unit-test:
   - a legal rook capture beside the enemy commoner is returned;
   - a capture far from both commoners is not;
   - a candidate target adjacent to our own commoner is rejected;
   - a commoner capturer is rejected;
   - an empty/no-capture move list returns `None`;
   - the returned move is one of the supplied legal moves.
3. **Hook node entry.** Add the flag-gated branch in `core.rs` at the stated
   insertion point. Add tests:
   - with the flag off, the same position's nodes/child evals are unchanged;
   - with the flag on, a one-move explosion root returns `Win` with
     `nodes == 1` and `child_evals == 0`;
   - the TT entry is `Win`, depth `1`, `best_move` is the winning capture,
     `best_child == u8::MAX`, `remaining_depth == u32::MAX`;
   - the proof event is `NodeProven(Win, depth 1)` at the node path;
   - `max_depth == 0` still returns `Draw` with no static claim;
   - a position with no explosion is not claimed.
4. **Add a regression for no false wins.** On a small non-explosion fixture,
   flag-on and flag-off runs must agree on the outcome; the flag may only
   reduce evals/nodes.
5. **Wire the flag.** Add the Search field/setter, `CliOptions` field/parser,
   help text/golden, and `main` wiring; parser tests for default-off and
   presence.
6. **Smoke and validate.** Build release. Run the core FEN and shared case
   with the flag off/on at short budgets; validate every decisive PV with
   `--tt-dump-path` + `reconstruct_pt` as in plan2/3. A claimed win that fails
   replay validation vetoes the lever.
7. **Measure.** Add `docs/plans/provari/measurements/plan4/` with
   `run_static.py`, `README.md`, and `results.json`. Reuse
   `../plan1/corpus.json` by path; never edit `sample20`.
8. **Run the gates.** `make test`, `make test-full`, the quick-suite drift
   check, and `cargo test --release --test test_repetition --
   --include-ignored`. With the flag off, every existing trajectory/golden
   test must pass unchanged.
9. **Write `report4.md`.** Include all repeats/censoring, the interaction
   tables, validation results, gate results, approximations, and one explicit
   default decision.
10. **Update the initiative.** Mark item 4a complete (or record a negative
    result), state whether 4b (exposed-defender/double-threat/no-safe-evasion)
    or a pivot is next, and update `docs/plans/README.md` only if the
    initiative pivots or closes.

## Measurement protocol

Release build, one process at a time, no tablebases, 192 MB TT label. Corpus
is the committed `measurements/plan1/corpus.json`.

Configurations (four, so the interaction with the current best stack is
visible):

| Name | Flags |
| --- | --- |
| `off` | shipped |
| `static` | `--static-tactics` |
| `best` | `--move-class-pn --quiet-grouping` (plan3's `both`) |
| `best+static` | `--move-class-pn --quiet-grouping --static-tactics` |

Command shape:

```sh
./target/release/atomic_solver --fen "<FEN>" --tt-size 192 \
  --first-outcome --outcome-only --timeout <T> [--budget <B>] [flags]
```

Runs:

- **Core FEN:** `--timeout 90 --budget 600000000`, 3 repeats per
  configuration. Report all repeats, medians, and whether `budget exhausted`
  or `timeout` fired.
- **Shared case `1eb80b92f92c`:** `--timeout 10 --budget 300000000`, 3 repeats
  per configuration.
- **`sample20`:** `--timeout 10` (no explicit budget; plan1 parity), 1 repeat
  per configuration. Keep every row and report censoring.

Primary metric: `child_evals` to the first decisive outcome (deterministic).
Secondary: wall time from the driver, outcome, first-win length, `pv_status`,
exit reason. On `sample20`, coverage and the on/off delta.
Budget-exhausted is never reported as a proven draw.

Validation: for every decisive row, rerun the same configuration with
`--tt-dump-path`, rebuild with `reconstruct_pt`, and require
`root_outcome == measured outcome` and `validate: ok`; deduplicate by
`(case, config, PV)`. `verify_ppv` may be run for completeness but is not the
acceptance criterion.

## Acceptance

- `--static-tactics` exists, defaults to off, is documented in `--help`, and
  is covered by parser/unit tests.
- With the flag off, `make test` and the existing trajectory/golden tests are
  unchanged; no behavior, TT layout, or proof-event change is detectable.
- With the flag on:
  - only a legal immediate exploding capture can produce the static `Win`;
  - the claim is `Win` depth 1 with the winning move as the TT hint;
  - terminal/repetition/rule50/depth-horizon behavior outside the claim is
    unchanged (the hook sits after those checks);
  - no `child_evals` are charged and no proof tree is built by the search;
  - every decisive measured win replays/validates.
- `measurements/plan4/` contains the driver, provenance README, and parsed
  `results.json`; no raw transcript, TT snapshot, or `__pycache__` is
  committed.
- `make test`, `make test-full`, the quick-suite drift check, and the
  repetition gate are green (or the report records a pre-existing failure
  with evidence).
- `report4.md` states one of: keep the flag default-off (negative/neutral),
  keep it opt-in and proceed to 4b, or adopt it as default (only with clear
  evidence and green gates).

## Validation / gates

- Default gate: `make test`.
- Search-behavior gate: `make test-full`.
- Repetition: `cargo test --release --test test_repetition -- --include-ignored`.
- Drift: `cargo run --release --quiet --example benchmark -- --suite quick`,
  recorded verbatim.
- Replay: the proof-tree reconstruct/validate path from plan2/3; `verify_ppv`
  is secondary.

## Risks and mitigations

- **The lever is neutral.** Most immediate explosions are already found by the
  first child evaluation and the terminal classification, so the static hook
  may only save a small number of child evals. Measure it alone and in the
  best stack; a neutral result closes 4a honestly.
- **A false win.** The target shape is conservative and the move comes from
  the legal move list. Add the negative fixtures (target far away, target
  beside our own commoner, commoner capturer) and the flag-off trajectory
  gate. Any validation failure vetoes the lever.
- **Placement drift.** Moving the hook above `max_depth`, repetition, or the
  TT solved check would change existing semantics. Pin the position with
  tests (`max_depth == 0` stays Draw; repetition stays Draw; a solved TT entry
  wins).
- **Overlapping with 4b.** Do not expand this plan into `exposed`/double
  threat/cannot-win if the primitive shows no signal; report and let the next
  plan re-rank.
- **Scope creep.** If the implementation starts needing a full
  `Mover::exposes`, safe-evasion filtering, or blast-aware exchange, stop and
  defer to 4b.

## Report requirements

`report4.md` is the final task and must include:

- the primitive and hook as implemented, with every approximation marked;
- the exact hook placement and the proof that flag-off is inert;
- the four-configuration core and shared repeat tables (all runs, medians,
  censoring);
- the `sample20` coverage table and deltas for each configuration;
- deterministic `child_evals` and wall comparisons, with the interaction with
  move-class and grouping;
- replay-validation results for every decisive row;
- gate results (`make test`, `make test-full`, repetition, drift);
- a recommendation: keep off, keep opt-in and proceed to 4b, or adopt as
  default;
- explicit "not measured" items (`exposed`/no-safe-evasion/double-threat,
  mate-in-one pre-scan, cannot-win material, EF-PN, tablebases, full
  `high200`).

## Later plans (context for 4b)

If 4a is positive or neutral, the next ranked sub-mechanism is provari's
`exposed` closure: a defender-to-move position where every legal reply allows
the attacker an immediate exploding capture is a Loss in two plies. It should
be evaluated as a child-side static result, with rule50 and path-repetition
guards, and measured against 4a's node-entry closure. Cannot-win material and
any dedicated mate-in-one scan remain separate candidates.
