# plan8 measurement artifacts

Raw outputs of the oracle-floor measurement (`docs/plans/nn/plan8.md`,
Step 2/3), captured on the reference container.

## Oracle tree generation (Step 2)

Candidate cases: the `move-order` suite minus `m20*`, `m21*`, `m22_black`
(14 cases; the plan's "13" was a miscount — the fixture has 19 cases and
19 − 5 excluded = 14). All 14 converged well under the 60 s budget, so
none were dropped for non-convergence.

```
mkdir -p data/oracle/trees
while IFS=';' read -r name fen; do
  ./target/release/atomic_solver --fen "$fen" --timeout 60 --first-outcome \
    --tt-size 64 --dump-path "data/oracle/trees/$name.bin" < /dev/null
done < oracle_cases.txt
```

Note: `docs/plans/nn/plan8.md` Step 2 shows `--outcome-only` in the
generation command, but that flag disables the pre-exit hook that writes
the dump; the command above omits it and redirects stdin instead.

`oracle_cases.txt` — name;FEN lines for the 14 generated trees
(`data/oracle/trees/` is git-ignored like `data/corpus/`).

## Files

- `decompose.txt` — `oracle_floor decompose` (static work decomposition,
  no search).
- `solve.txt` / `solve.stderr.txt` — `oracle_floor solve` (baseline vs
  oracle-ordered searches; stderr contains the bounded-search chunk log).
- `fractions_check.txt` — `move_order_fractions --suite move-order`
  harness-drift guard re-run (default settings, as in report1/report6).

## Harness-drift guard result

Per-case `move_order_fractions` results are bit-identical to
`docs/plans/nn/measurements/gate4b/fractions_baseline.txt` for every
non-timeout-bound case (m23_white … m29_white: same tree_nodes,
or_nodes, timeout flags). The aggregate differs (10194/14806 = 68.9%
flat rank-1 here vs 10334/14863 = 69.5% there; work-weighted 31.4% in
both) because m20–m22 hit the 5 s default timeout and their partial
trees depend on host speed — an environmental difference, not harness
drift. The `Search::sort_moves` static path is bit-identical (the
ordering-scorer hook only activates when an override is set).
