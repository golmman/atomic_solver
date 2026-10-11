# plan20 measurements — salted TT-keeping restarts (research #20, Phase-0 PIVOT)

Provenance: repo HEAD `546bbcc` ("add research plan20") + the temporary
`ATOMIC_RESEED` hook (`TranspositionTable::reseed` in
`src/search/tt/table.rs` + the `Search::reseed_at_boundary` call site in
`src/search/dfpn/mod.rs`, applied, measured, reverted in the same session;
`git diff --exit-code` verified). Host: x86_64 container, 4 cores. Build:
release, LTO thin.

**No rollout was executed**: the pre-registered Phase-0 kill gate K1
(retention) fired on every stress-class probe, so the item closed at
Phase 0 per plan20's Gates. The `driver.py`/`parse.py`/`verdicts.py`
triplet is the pre-registered (verbatim-adapted) tooling; it was not
exercised end-to-end (the pure functions were smoke-tested —
`parse_run` against probe transcripts and `classify_verdict` against
synthetic per-salt records; the D3(b)/(d) hard-gate code paths are the
plan19-reviewed logic adapted verbatim).

| file | role | committed |
|---|---|---|
| `driver.py` | pre-registered rollout driver (22 cases × 6 salts × {base, rs1, rsg}); **not run** (no Phase 1) | yes |
| `parse.py` | pre-registered raw → `state/` parser with the D3(b)/(d) hard gates + reseed-count profile; **not run** (no rollout) | yes |
| `verdicts.py` | pre-registered pinned-v1.0 verdict machinery (two candidate arms + combined verdict); **not run** (no rollout) | yes |
| `env.json` | run commands, hook spec, D3(a)/(c) fidelity record, Phase-0 probe results, the K1 retention table, kill-gate outcome, reseed-count profile | yes |
| `state/` | (empty) — parse.py/verdicts.py outputs would live here; no rollout produced any | — |

Raw transcripts (`/tmp/plan20/...`: pristine identity runs, anchor chain,
probe runs, cyclic-rook invariant runs) are not committed (`.gitignore`d
per the AGENTS.md measurement layout); `env.json` records the parsed
results, and the anchors are regenerable one-liners.
