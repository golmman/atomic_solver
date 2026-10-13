# plan12 measurements — Phase-0 noise probe (backlog #7 groundwork)

Executes `plan12.md` stage 0 (read-only; no code changes). The re-land
(stage 1), per-phase accounting (stage 2), and revert (stage 3) are
GO-branch-only; D1 fired **NO-GO**, so they never ran and this directory
holds probe outputs only.

Provenance: plan6 campaign data (`../plan6/state/campaign6_raw.jsonl`,
8 rounds × N ∈ {1..4} × {m22, rem12, shuffle_win}, with per-run
helper/coordinator evals) judged against the sequential cross-salt
realization bands: the committed research-plan12 corpus (22 cases × 6 salts;
`stress` = shuffle-win, `m22_white` = m22; rem12 absent) plus two fresh
current-binary probes (rem12 all 6 salts; stress salts {0,1,5} as a
drift-robustness check). No parallel runs, no code changes; the solver
writes no files in these runs (raw transcripts under `/tmp/plan12_probe`,
gitignored-by-location).

| file | what |
| --- | --- |
| `env.json` | environment, binary-drift declarations, data sources |
| `rem12_salts.py` / `state/rem12_salts.json` | fresh rem12 6-salt realization distribution (all decisive; band [0.963, 1.088], spread 1.129×) |
| `stress_salts_fresh.py` / `state/stress_salts_fresh.json` | fresh stress salts {0,1,5} — loud salt-1 draw persists on the current binary (3.95× vs corpus 3.04×) |
| `probe_noise.py` / `state/probe_noise.json` | the mechanical H0 probe: magnitude (max tail inflation vs ratio × overhead products, three overhead estimators), frequency (veto-only), helper/coordinator decomposition |

Headline (details in `report12.md`): **H0 not rejected on both
pre-registered cases** — the shuffle-win t4 bad tails (5.67/5.93/6.33×)
sit below every `ratio_max × overhead` product (≥ 7.50), the decomposition
shows a near-constant mechanism work multiplier (~3.7–3.9×) with the tail
explained by mid-band coordinator realization draws, and the frequency test
cannot veto (sequential stress tail rate 1/3 uncensored, 4/6 all draws, vs
parallel 3/8). rem12 has no tails; its 3.03× median inflation is the known
mechanism overhead, not realization luck. **D1 = NO-GO → backlog #7 closed;
plan13 never drafted.** Out-of-scope finding: m22's t3/t4 tails (11.5× /
33.9×) DO exceed every noise product (~7.1× max) — a parallelism-specific
inflation component exists on the short control case (perturbation /
repetition-stall signature), consistent with the plan6 NO-GO record; it does
not rescue #7's long-case premise (S4(m22) = 1.01×).
