# Plan 11 measurements (execution session 2026-10-10)

Implements backlog #18 (`plan11.md`): `TranspositionTable::prefetch` +
`evaluate_all_children` pre-pass, exactly the phase-0 spike V10
(`../plan11_phase0/spike_prefetch_prepass.patch`).

**Location note (2026-10-10):** originally committed at repo-root
`measurements/plan11/` by the plan11 session; relocated here (byte-
identical, re-verified against raw sources) after report12 flagged the
non-conventional path. See `report11.md`'s post-commit correction.

**Host caveat:** the phase-0 spike/acceptance numbers (−51% / −55% / −48%)
were measured on the aarch64 reference VM; this session ran on an x86_64
container (see `env.json`). All numbers below are interleaved A/B pairs on
this host; stdout identity was verified by md5 for every run.

## Files

| file | content |
| --- | --- |
| `quick_before.json` | `benchmark --suite quick --json --first-outcome --timeout 3 --runs 1`, HEAD binary (task 1 baseline) |
| `state/baseline.json` | baseline md5s/walls for m22 default, m22 FO, shuffle-win FO, quick-suite aggregates; golden check |
| `shuffle_fo_base_leaves.txt` | `perf report --no-children` leaf table, HEAD binary, shuffle-win FO, `--timeout 20` |
| `shuffle_fo_post_plan11_leaves.txt` | leaf table, plan11 binary, same workload (task 6) |

## Results (x86_64 host, sequential interleaved A/B)

| workload | HEAD wall | plan11 wall | Δ | stdout md5 |
| --- | --- | --- | --- | --- |
| m22 default ×5 | 3.33–3.45 s (mean 3.42) | 2.56–2.64 s (mean 2.60) | −23.8% | identical ×5/×5 |
| m22 FO ×5 | 2.75–2.86 s (mean 2.77) | 2.10–2.12 s (mean 2.11) | −23.8% | identical ×5/×5 |
| shuffle-win FO ×2 | 46.04 / 46.43 s | 34.06 / 33.99 s | −26.4% | identical ×2/×2 (`cfc58bc4…`) |
| quick suite (search time) | 6.68 s | 5.52 s | −17.3% | 59/59 identical (`child_evals`/`nodes`/`outcome`) |

All below the phase-0 acceptance (−30%) but strictly beneficial; the gap is
attributed to the host change (see `report11.md`). V12-style double-pre-pass
probe: 2.60 → 2.96 s on m22 default ⇒ pre-pass share ≈14% of post-plan11
wall (phase-0 estimate ≈12%) → #19/plan12 still justified.
