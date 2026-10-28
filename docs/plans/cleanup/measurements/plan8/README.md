# plan8 measurements — CLI layer verbosity

Cosmetic refactor of the CLI entry layer (`src/cli.rs`, `src/main.rs`);
no search code is touched, so every anchor is expected to be
child-eval identical and the `--help` / stdout contracts
byte-identical. Provenance per `env.json`.

## Files

| File | What it is |
| --- | --- |
| `env.json` | commit, host, rustc/cargo, binary/profile |
| `state/help_before.txt` | HEAD `--help` stdout (argv[0] normalized to `atomic_solver` via `exec -a`) |
| `state/help_after.txt` | same after Phases A–D — `diff` empty |
| `state/stdout_default_before.txt` / `_after.txt` | dec31 FEN, default mode, **stdout only** (the `preflight:` / `outcome:` / `pv:` / `pv_status:` / `pre_exit:` contract) |
| `state/stdout_outcome_only_before.txt` / `_after.txt` | dec31 FEN, `--outcome-only`, stdout only |
| `state/stderr_default_before.txt` / `_after.txt` | same run, **stderr only** (the `evals:` line) |
| `state/stderr_outcome_only_before.txt` / `_after.txt` | idem, `--outcome-only` |
| `state/wc_before.txt` / `state/wc_after.txt` | `wc -l src/cli.rs src/main.rs` (+ `src/lib.rs` after) |
| `state/git_status_before.txt` | worktree state at Phase 0 |
| `state/quick_after.json` | `benchmark --suite quick … --json` (anchor 1) |
| `state/anchors_after.json` | parsed anchor values (quick total, dec31, m22) |

**stderr caveat.** Each stderr capture contains one
`[bounded_search] chunk done: … elapsed=… nps=…` diagnostic line whose
`elapsed`/`nps` fields vary per run (confirmed: two runs of the *same*
HEAD binary differ only there). Comparisons are made on the captures
with those lines filtered out; the `evals:` line is identical.

The "before" binary was built from `git archive HEAD` into
`/tmp/plan8_before` so the comparison is HEAD-vs-worktree, not
remembered numbers.

## Reproduce

```bash
# before
rm -rf /tmp/plan8_before && mkdir -p /tmp/plan8_before
git archive HEAD | tar -x -C /tmp/plan8_before
(cd /tmp/plan8_before && CARGO_TARGET_DIR=/tmp/plan8_before/target \
  cargo build --release --bin atomic_solver)

# after
cargo build --release --bins --examples

S=docs/plans/cleanup/measurements/plan8/state
FEN='r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15'
for side in before after; do
  [ $side = before ] && BIN=/tmp/plan8_before/target/release/atomic_solver \
                     || BIN=./target/release/atomic_solver
  bash -c "exec -a atomic_solver $BIN --help" > $S/help_$side.txt
  $BIN --fen "$FEN" --timeout 20 --first-outcome </dev/null \
      > $S/stdout_default_$side.txt 2> $S/stderr_default_$side.txt
  $BIN --fen "$FEN" --timeout 20 --first-outcome --outcome-only </dev/null \
      > $S/stdout_outcome_only_$side.txt 2> $S/stderr_outcome_only_$side.txt
done
diff $S/help_before.txt $S/help_after.txt                       # empty
diff $S/stdout_default_before.txt $S/stdout_default_after.txt   # empty
diff $S/stdout_outcome_only_before.txt $S/stdout_outcome_only_after.txt  # empty
diff <(grep -v '^\[bounded_search\]' $S/stderr_default_before.txt) \
     <(grep -v '^\[bounded_search\]' $S/stderr_default_after.txt)        # empty

# anchors
./target/release/examples/benchmark --suite quick --first-outcome --timeout 3 \
  --runs 1 --tt-size 256 --json > $S/quick_after.json
./target/release/atomic_solver --fen "$FEN" --timeout 20 --first-outcome \
  --outcome-only </dev/null                       # evals: 510426
./target/release/atomic_solver --timeout 60 --first-outcome --outcome-only \
  --fen '4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22' </dev/null
```

## Results

| anchor | expected | measured |
| --- | --- | --- |
| quick @ 256 | 59 cases, 42,109,768 child evals, 0 wrong, 0 timeouts | 59 / 42,109,768 / 0 / 0 |
| dec31 FO | `evals: 510426`, `outcome: loss length: 22` | identical |
| m22 FO | `evals: 15023911`, `outcome: win length: 75` | identical |
| `--help` bytes | HEAD | identical (2,601 B) |
| dec31 stdout (both modes) | HEAD | identical |
| dec31 stderr (`evals:`) | HEAD | identical (diagnostic line excluded, see caveat) |

Line counts: `src/cli.rs` 488 → 396, `src/main.rs` 360 → 308
(total 848 → 704; the plan's ≤660 target is a drafting mis-estimate —
see `report8.md` §5).
