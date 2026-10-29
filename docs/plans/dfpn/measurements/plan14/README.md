# measurements/plan14 — cut-context table Phase 0

Provenance for the plan14 Phase 0 spike (`docs/plans/dfpn/plan14.md`) and its
reversible archive. The spike was temporary and has been reverted; `src/` is
byte-identical to `HEAD` (`git diff --exit-code -- src/` clean after the
revert), and every non-censored salt-0 anchor reproduced exactly
(`state/matrix_post_revert.json`).

| File | What it is |
| --- | --- |
| `plan14_matrix.py` | The measurement driver: runs the release CLI over the case list and arm grid (env-gated spike), parses `outcome:`, `evals:` and the `cutctx:` counter line, appends parsed results to JSON. No raw transcripts are kept (AGENTS.md). |
| `env.json` | Host, toolchain, binary flags, TT label ↔ bucket mapping, spike gate variables, memory labels. |
| `state/matrix_spike.json` | The full Phase 0 matrix (8 cases × [2 baselines + 4 spike arms]) with per-arm cut-context counters. |
| `state/matrix_m20.json` | The hard-class `m20_white` control at the candidate shape (base vs `256 MB / 2^22`). |
| `state/matrix_post_revert.json` | The post-revert salt-0 baseline re-run (8 cases × 128/256 MB), compared against the spike build's feature-off baseline; all non-censored cells exact. |
| `state/quick_off.json`, `state/quick_on.json` | `benchmark --suite quick --json --first-outcome`, feature off vs `DFPN_CUTCTX=1 DFPN_CUTCTX_SLOTS=22` (default 256 MB main). |
| `spike_src.diff` | The complete spike diff for the five modified files (`mod.rs`, `config.rs`, `run.rs`, `core.rs`, `tests.rs`). |
| `cut_context.rs.spike` | The new spike module (`Search::cut_context` side table, env gate, counters, unit tests). Applying both files (`patch -p1 < spike_src.diff` + copy) restores the measured build. |

## Commands

```bash
# Baseline + matrix (feature off, then DFPN_CUTCTX=1 at 128/256 MB and 2^21/2^22 slots)
python3 docs/plans/dfpn/measurements/plan14/plan14_matrix.py --arms base,on --tt 128,256 --slots 21,22

# m20 control at the candidate shape
python3 docs/plans/dfpn/measurements/plan14/plan14_matrix.py \
  --only m20 --arms base,on --tt 256 --slots 22 \
  --out docs/plans/dfpn/measurements/plan14/state/matrix_m20.json

# Quick suite (59 cases)
env -u DFPN_CUTCTX cargo run --release --example benchmark -- \
  --suite quick --json --first-outcome > .../state/quick_off.json
DFPN_CUTCTX=1 DFPN_CUTCTX_SLOTS=22 cargo run --release --example benchmark -- \
  --suite quick --json --first-outcome > .../state/quick_on.json

# Soundness / termination (spike in tree)
DFPN_CUTCTX=1 DFPN_CUTCTX_SLOTS=22 cargo test --release --test test_repetition -- --include-ignored
cargo test --release --lib cut_context
```

`plan14_matrix.py --out PATH` overrides the output file; the default is
`state/matrix.json`. Runs append, so remove/rename the target to start fresh.
