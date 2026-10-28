# plan26 measurements — the flip OR-witness semantics (2026-10-27)

Executes `../../plan26.md`'s probe and evidence tasks: the §0.4 probe table
re-run **after** the change (both live layers must be unchanged), the pre-fix
red messages of the two synthetic CLI fixtures (captured against the
unmodified binary), and their post-fix JSON reports. Every run is
regenerable from the commands shown; no transcripts are committed (the
stdout/stderr excerpts below are short verbatim quotes).

Environment: the dev container, `cargo build --release --examples`
(`target/release/examples`), standing layer `data/proofdb/proofdb.db`
(43,092 nodes), committed fixture fold `docs/plans/proofdb/shards`
(55,703 nodes, fold digest `14fd07530996eb9e`).

## §0.4 probes re-run after the change

```bash
BIN=target/release/examples
mkdir -p /tmp/p26/shards_empty
$BIN/proofdb_flip --db data/proofdb/proofdb.db --shard-dir /tmp/p26/shards_empty \
    --out /tmp/p26/flip_standing.json
cp -r docs/plans/proofdb/shards /tmp/p26/fx && \
  $BIN/proofdb_merge --manifest /tmp/p26/fx/manifest.json --shard-dir /tmp/p26/fx \
    --db /tmp/p26/fx.db && \
  $BIN/proofdb_flip --db /tmp/p26/fx.db --shard-dir /tmp/p26/fx --out /tmp/p26/flip_fixture.json
```

| layer | nodes | open rows | pre-fix stdout | post-fix stdout |
| ----- | ----- | --------- | -------------- | --------------- |
| standing | 43,092 | 64 | `flip_analysis: open_rows 64 flips 0 verified 0 root Null (fixpoint 1 rounds)` | `flip_analysis: open_rows 64 flips 0 verified 0 root Null (fixpoint 1 rounds; or-witness 0, and-complete 0, exact 0)` |
| committed fixture fold | 55,703 | 75 | `flip_analysis: open_rows 75 flips 0 verified 0 root Null (fixpoint 1 rounds)` | `flip_analysis: open_rows 75 flips 0 verified 0 root Null (fixpoint 1 rounds; or-witness 0, and-complete 0, exact 0)` |

Both exit 0; both `flips 0`, and every pre-existing JSON key keeps its value
(`open_rows`, `flips: []`, `flips_total 0`,
`flips_verified_independently 0`, `root_implied null`, `root_bound null`); the
only delta is the three additive counters and the stdout suffix. The
semantics change is therefore invisible on both live layers (their open rows
are walk-expansion spines with 0 or 1 resolved child — no proven loss child).

## The cited production shape is legal (probe re-check)

```bash
$BIN/list_legal --fen 'rnbqkbnr/pp2p1pp/2pp4/1N3pN1/8/8/PPPPPPPP/R1BQKB1R w KQkq - 0 5'
```

30 legal replies, `a2a3` **first** in movegen order, `b5c7` legal — so the
production `reply a2a3 has no DB row` message reproduces exactly in the
synthetic fixture.

## D4 red capture (pre-fix binary, unmodified tool)

`cargo test --release --test proofdb_flip` on the pre-change tree — both
synthetic CLI fixtures fail (exit 1 each):

```text
or_witness_flip_reports_coverage_and_bound:
  proofdb_flip: FLIP VERIFICATION FAILED: flip "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 c3b5 f6f5": reply a2a3 has no DB row
or_bound_is_recomputed_to_the_fixpoint:
  proofdb_flip: FLIP VERIFICATION FAILED: flip "": want Flip { stm_win: true, bound: 11 }, verified Flip { stm_win: true, bound: 4 }
```

The first is the plan8 §4.5 AND-completeness gate applied to an OR claim (the
item-24 incident); the second is the derive-once bound divergence (round-1
bound 11 vs the converged 4). Both messages are byte-identical to the plan's
prediction.

## Post-fix CLI fixture reports

`or_witness_flip_reports_coverage_and_bound` — exit 0, stdout
`flip_analysis: open_rows 9 flips 1 verified 1 root Null (fixpoint 2 rounds; or-witness 1, and-complete 0, exact 0)`:

```json
{
  "flips": [
    {
      "bound": 23,
      "bound_mode": "upper",
      "coverage": "or-witness",
      "fixpoint_round": 1,
      "implied": "root-win",
      "loss_witnesses": 1,
      "path": "g1f3 d7d6 b1c3 c7c6 f3g5 f7f6 c3b5 f6f5",
      "ply": 8,
      "side_to_move": "win",
      "unresolved_replies": 29
    }
  ],
  "flips_and_complete": 0,
  "flips_exact": 0,
  "flips_or_witness": 1,
  "flips_total": 1,
  "flips_verified_independently": 1,
  "open_rows": 9,
  "root_bound": null,
  "root_implied": null
}
```

`or_bound_is_recomputed_to_the_fixpoint` — exit 0, stdout
`flip_analysis: open_rows 2 flips 2 verified 2 root root-win (fixpoint 3 rounds; or-witness 0, and-complete 2, exact 0)`
(the converged root claim is bound **4**, not the round-1 11):

```json
{
  "flips": [
    { "bound": 4, "bound_mode": "upper", "coverage": "and-complete", "fixpoint_round": 1,
      "implied": "root-win", "loss_witnesses": 2, "path": "", "ply": 0,
      "side_to_move": "win", "unresolved_replies": 0 },
    { "bound": 3, "bound_mode": "upper", "coverage": "and-complete", "fixpoint_round": 1,
      "implied": "root-win", "loss_witnesses": 0, "path": "a2a3", "ply": 1,
      "side_to_move": "loss", "unresolved_replies": 0 }
  ],
  "flips_and_complete": 2,
  "flips_exact": 0,
  "flips_or_witness": 0,
  "flips_total": 2,
  "flips_verified_independently": 2,
  "open_rows": 2,
  "root_bound": 4,
  "root_implied": "root-win"
}
```

The second flip is the open child itself (all 20 of its replies are resolved
wins for their mover, so it claims a side-to-move loss of bound 3) — the plan
text mentioned `verified 1` for this fixture, which counted the root only;
the fixture asserts the root entry's fields plus `verified == flips_total`.

`bound_mode upper` on both and-complete claims is the honest D2 rule: every
contributing child is `depth_status = 'bound'` (the synthetic rows), so
minimality is not derivable.

## File sizes (D5 split)

| file | before | after |
| ---- | ------ | ----- |
| `examples/proofdb_flip.rs` | 13,322 | 5,391 |
| `examples/proofdb/flip/mod.rs` | — | 7,949 |
| `examples/proofdb/flip/fixpoint.rs` | — | 7,051 |
| `examples/proofdb/flip/verify.rs` | — | 8,766 |
| `examples/proofdb/flip/tests.rs` | — | 7,663 |
| `tests/proofdb_flip.rs` | — | 8,963 |

The tool's 12 KB size-justification header is retired; `flip/fixpoint.rs` is
an added split relative to the plan's D5 (mod.rs alone exceeded 10 KB with the
normative header and the converged pass in one file).
