# Plan 18 measurement artifacts

Re-score of the `lean` plan10 history/killer arms under the pinned gate
(research #19, third arm), release build, 22-case frozen plan12 corpus ×
6 salts × 3 arms = 396 runs, `--first-outcome`, aarch64 container host,
2026-10-11. Instrumentation: temporary env-gated history/killer constant
overrides (`LEAN10_HISTORY_BONUS` / `_MAX` / `_AGE_INTERVAL`, the lean
plan10 constant-override pattern, defaulting to the `history.rs` consts) plus
the temporary `LEAN10_STATS=1` sort_moves reorder diagnostic, applied on top
of HEAD (`2f5e86e`), measured, and **fully reverted** — `git diff` empty, and
the post-revert salt-0 anchors re-run exact. Raw transcripts live under
`/tmp/plan18/results` (not committed per AGENTS.md); the parsed record is
authoritative.

## Files

- `driver.py` — the rollout driver (resumable; arms selected by the env hook;
  baseline arm run first and identity-checked per D3).
- `parse.py` — parses raw outputs into `state/runs.json` +
  `state/summary.json`, enforces D3(b) (132-cell baseline per-salt
  bit-identity vs plan12) and D3(d)/structural (outcome invariants).
- `verdicts.py` — pinned v1.0 verdict machinery applied verbatim
  (`state/verdicts.json`), with the paired-ratio dispersion summary (the
  comparison rule's fourth real-arm calibration data point).
- `env.json` — environment, exact commands, arm definitions, and the recorded
  D3(a)/(c) pre-rollout fidelity anchors (pristine salt-0 identity, liveness
  probes, env-unset quick byte-identity, counter anchor, candidate-arm quick
  fidelity).
- `state/` — committed parsed results (`runs.json`, `summary.json`,
  `verdicts.json`).

## Commands

```bash
# rollout (per run; arm = env hook, command line identical)
LEAN10_<CONST>=<v> target/release/atomic_solver --fen <FEN> --tt-size 128 \
  --first-outcome --outcome-only --salt <s> --budget <cap> --timeout 600

# D3(c) quick anchors (hook build, before the rollout)
target/release/examples/benchmark --suite quick --first-outcome --timeout 3 --runs 1 --json
LEAN10_STATS=1 target/release/atomic_solver --fen <quick FEN> --timeout 3 \
  --first-outcome --outcome-only          # stderr: lean10stats: counters

# D3(f) cyclic-rook invariant (per candidate arm, --no-preflight)
LEAN10_<CONST>=<v> target/release/atomic_solver --fen '8/8/8/8/2k5/8/8/4KR2 w - - 0 1' \
  --tt-size 128 --first-outcome --outcome-only --timeout 600 --no-preflight
```

## Anchor numbers

- D3(a) pristine salt-0 identity (pre-hook HEAD build): stress 249,480,478;
  m22 14,156,269; m23 9,673,403; dec13 3,822,602; dec10 4,262,128; m20
  censored at 2 B — all exact vs plan12; m22 stdout md5 `ea72f7ea…`.
- D3(c0) liveness: lean plan10 probe arms `ag1` (31,493,500) and `hb1`
  (38,373,893) reproduce bit-exactly on the hook build (58 ok + the archived
  timeout case each, zero per-case diffs on ok cases).
- D3(c1): env-unset hook build quick suite byte-identical to the archived
  step-0 baseline (38,974,090; 59/59 per-case child evals + statuses +
  outcomes).
- D3(c2) counter anchor: 2,477,336 `sort_calls` / 482,956 `reorder_diffs`
  reproduce the archived lean plan10 `LEAN10_STATS` diagnostic exactly
  (child-eval total unchanged at 38,974,090 — side-band confirmed).
- D3(c3): `hs2ag1` (31,345,410) and `hs05ag1` (34,631,157) quick totals
  bit-exact vs the archived arm JSONs; `m23_white` reproduces
  `timeout: true` under both arms (wall-capped evals not deterministic, per
  the pre-registered limitation).
- D3(f): cyclic rook proves `Draw` under both arms with `--no-preflight`
  (never `Win`).

See `report18.md` for verdicts.
