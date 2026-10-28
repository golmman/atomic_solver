# plan9 measurements — `dfpn/children.rs` constructors, bounds helper, test split

Behaviour-free refactor of the hottest file in the program. Every anchor is
expected to be child-eval identical; the wall cells are a sanity bar, not a
performance claim. Provenance per `env.json`.

## Files

| File | What it is |
| --- | --- |
| `env.json` | commit, host, rustc/cargo, binary/profile, before-binary path |
| `state/wc_before.txt` / `state/wc_after.txt` | `wc -l -c` for the production file and the new test sibling |
| `state/census_before_after.txt` | `ChildInfo {` literals, `repetition_seen: false`, `.pn_dn_for(..).0/.1`, ladder bodies, `#[test]` count |
| `state/quick_before_tt256.json` / `state/quick_after_tt256.json` | full `benchmark --suite quick … --json` runs at TT 256 |
| `state/quick_before_tt128.json` / `state/quick_after_tt128.json` | same at TT 128 |
| `state/summary.json` | parsed quick totals, anchor values, wall medians |
| `state/wall_interleaved_7rep.txt` | HEAD-vs-after interleaved wall medians |
| `state/git_status_before.txt` | worktree state at Phase 0 |

The "before" binary was built from `git archive HEAD` into
`/tmp/head-target`, so the wall comparison is HEAD-vs-worktree on the same
host, not remembered numbers.

## Reproduce

```bash
# before
rm -rf /tmp/head-src && mkdir -p /tmp/head-src
git archive HEAD | tar -x -C /tmp/head-src
(cd /tmp/head-src && CARGO_TARGET_DIR=/tmp/head-target cargo build --release)

# after
cargo build --release --bins --examples

S=docs/plans/cleanup/measurements/plan9/state
./target/release/examples/benchmark --suite quick --first-outcome --timeout 3 \
  --runs 1 --tt-size 256 --json > $S/quick_after_tt256.json
./target/release/examples/benchmark --suite quick --first-outcome --timeout 3 \
  --runs 1 --tt-size 128 --json > $S/quick_after_tt128.json

D='r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15'
M='4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22'
W='4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23'
./target/release/atomic_solver --fen "$D" --timeout 20 --first-outcome
./target/release/atomic_solver --fen "$M" --timeout 60 --first-outcome
./target/release/atomic_solver --fen "$W" --timeout 60 --first-outcome
./target/release/atomic_solver --fen "$M" --timeout 20 --outcome-only \
  | diff -u tests/fixtures/m22_default_stdout_golden.txt -
```

## Results

| anchor | expected | measured |
| --- | --- | --- |
| quick @ 256 | 59 cases, 42,109,768 child evals, 0 wrong, 0 timeouts | 59 / 42,109,768 / 0 / 0 |
| quick @ 128 | 59 cases, 40,042,745 child evals, 0 wrong, 0 timeouts | 59 / 40,042,745 / 0 / 0 |
| dec31 FO | `evals: 510426`, `outcome: loss length: 22` | identical |
| m22 FO | `evals: 15023911`, `outcome: win length: 75` | identical |
| m23_white FO | `evals: 11621376`, `outcome: win length: 27` | identical |
| m22 default golden | passes, **not** re-baselined | passes |

Wall (7 interleaved reps, medians): dec01 0.9216 → 0.9289 s (1.008×),
dec05 0.0163 → 0.0156 s (0.960×), m22 2.1037 → 2.1228 s (1.009×) — all
within the plan's +2% bar.

Census: `ChildInfo {` 15 → 5 (struct + 3 fn signatures; the literals are
gone), `repetition_seen: false` 10 → 2 (the two neutral constructors),
`.pn_dn_for(..).0/.1` pairs 3 → 0, unsolved-bounds ladder bodies 2 → 1.
Production file 1334 lines / 57,771 B → 624 lines / 29,210 B; tests
649 lines / 24,155 B in `children/tests.rs`; 21 `#[test]` names before
and after.

## Note on the test split

The moved body was dedented verbatim, then `cargo fmt` (required by the
gate) rejoined exactly two `evaluate_child_played(...)` argument lists
that now fit the width at the shallower indentation. The only textual
differences from the normalized HEAD test module are those two
reflows plus the removed `#[cfg(test)] mod tests { … }` wrapper; no test
assertion or fixture changed.
