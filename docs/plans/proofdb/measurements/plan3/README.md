# plan3 measurements — coverage-policy A/B (2026-09-29)

Item 4 per `../../plan3.md`: the harvest job selection extended from
"open nodes only, deepest-first" to pre-registered policies over the *full*
frontier (C1 open / C2 unexpanded siblings of proving children / C3
unexpanded children of open nodes), compared at a fixed 300M child-eval
screen budget each, each policy a separate process with a fresh TT over the
re-merged input DB (marginal-yield design).

## Command table (in run order)

| file | command |
| --- | --- |
| `proofdb_input.db`, `nodes_input.txt` | the A/B input DB: `target/release/examples/proofdb_merge --manifest ../../shards/manifest.json --shard-dir ../../shards --db proofdb_input.db --dump nodes_input.txt` (107 shards; byte-identical to plan2's `proofdb_grown.db`, sha256 `949d6e59…`; the H3 diff base) |
| `p0_stdout.log`, `p0_stderr.log` *(not committed)* | P0 `open-deepest`: `proofdb_harvest --db proofdb_input.db --manifest ../../shards/manifest.json --shard-dir ../../shards --policy open-deepest --heavy-sample 0 --max-total-evals 300000000 --stop-file /tmp/plan3_STOP` → 75 screen jobs, 0 decisive, 300.0M evals, 64.7 s, stop=exhausted |
| `proofdb_after_p0.db` *(deleted: byte-identical to `proofdb_input.db`)* | `proofdb_merge` after P0 (no new shards) |
| `p1_stdout.log`, `p1_stderr.log` *(not committed)* | P1 `sharp-siblings`: same command with `--db proofdb_input.db --policy sharp-siblings` → 385 C2 screen jobs, 90 decisive (90 new shards), 300.1M evals, 72.9 s, stop=budget |
| `merge_p1.log` *(not committed)* | `proofdb_merge` after P1: 197/197 shards, 35,055 nodes, conflicts 0 |
| `proofdb_after_p1.db`, `nodes_after_p1.txt` | P2's input DB (= the merge after P1; byte-identical to the final `proofdb_grown2.db` since P2 added nothing — kept under the after_p1 name as P2's exact input) |
| `p2_stdout.log`, `p2_stderr.log` *(not committed)* | P2 `sharp-heavy-tail`: same command with `--db proofdb_after_p1.db --policy sharp-heavy-tail` (heavy defaults: sample 5, budget 40M) → 300 C2 screen jobs, 0 decisive; heavy tier 5 × 40M, 0 decisive; 300.0M + 200M evals, 105.5 s, stop=budget |
| `merge_p2.log`, `merge_grown2_a.log`, `merge_grown2_b.log` *(not committed)* | `proofdb_merge` after P2, then **twice** over the grown manifest (H2): DB sha256 `670e19e3…` both, dump `e3060d14…` both |
| `proofdb_grown2.db`, `nodes_grown2.txt` | the final merged DB (197 shards, 35,055 nodes = 34,980 proven + 75 open) + canonical dump (`--sample-lines 10` output in `sample_lines.txt`) |
| `assemble_census.py` | driver: parses the three transcripts into `census_<policy>.json`, runs the H3 diff (input vs grown2 per path) and the H4 manifest↔DB spot-checks, writes `policy_comparison.json` |
| `census_open-deepest.json`, `census_sharp-siblings.json`, `census_sharp-heavy-tail.json` | per-policy job records (path, policy, class, parent_bound, tier, budget, child_evals, wall, outcome, exit_reason, tag, shard_nodes) + summary metrics |
| `policy_comparison.json` | the pre-registered metric table (facts per 100M evals; cost/fact distribution; per-class decisive rates; ply distribution) + winner |
| `sample_lines.txt` | the merger's `--sample-lines 10` output over the grown2 DB (H4 informational replays) |
| `env.json` | captured at run time (git rev, cgroup limits, date) |

## Result (the pre-registered metric)

| policy | screen jobs | new facts | screen evals | facts / 100M | cost/fact (median) | verdict |
| --- | --- | --- | --- | --- | --- | --- |
| P0 `open-deepest` | 75 C1 | 0 | 300.0M | 0.0 | — | baseline: the plan2-depleted C1 plateau is undecidable at 4M |
| **P1 `sharp-siblings`** | 385 C2 | **90** (all ply-5 losses) | 300.1M | **29.99** | 44.7k evals | **winner** (decision 7: now the default `--policy`) |
| P2 `sharp-heavy-tail` (marginal, after P1) | 300 C2 + 5 heavy | 0 | 300.0M + 200M | 0.0 | — | the C2 gradient saturates immediately after P1's head |

The 90 facts are exactly the hypothesized coverage class: every unexpanded
sibling of a mate-in-1 proving child (`… d1a4 <try>` at 1.f3 e6 2.g4 — each
a refutation proving the try throws the win away). All 90 have
`parent_bound = 1`; all are `loss` facts at ply 5; cheapest cost 65 evals.
H5 replay: 385/385 screen records identical. H1–H4, H6: pass (see
`report3.md` for verdicts and findings).

## Gate results

- **H2** merge-after-batch determinism: two full merger runs → DB `670e19e3…`
  and dump `e3060d14…` byte-identical.
- **H3** no-regression vs input: 12,655 input nodes (12,580 proven) — 0
  outcome flips, 0 bound increases, no proven node became open (driver
  output).
- **H4** new-fact spot-checks: 90/90 new shard roots DB↔manifest agreement;
  skeleton re-validations 197/197 in the final merge; `--sample-lines 10`
  recorded.
- **H5** policy determinism: P1 re-run from `proofdb_input.db` + fresh TT
  (throwaway manifest/shard-dir at the pre-P1 manifest state, rebuilt by
  excluding the 90 census tags → digest `61866d6c…` exact) → 385/385
  screen records identical (all fields except wall time).
