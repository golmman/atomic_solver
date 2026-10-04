# Plan 11 measurements — Phase 3 Stage 2a: advisory-dn censor-signal
# validation at larger n + the pre-registered certificate soundness contract

Executes `docs/plans/solve/plan11.md` (Stage 2a; the M2 gate material for
plan12). Driver: `sweep11.py` (Python 3 stdlib only), following the plan10
`sweep.py` conventions (driver-written job files, bare `campaign_worker`
processes, master bypassed, per-rung close = barrier → lowercase `stop`
file → bounded wait for exits + complete TT-dump set, warm relaunch with
`--tt-load`, tree-RSS sampler, oom watch, crash recovery). The final merge
pass (`campaign_master --resume`, `--job-seed 11`) re-verifies every
decisive result under the global context (A2 discipline) over plan10's
committed merged state.

## Command table

| command | purpose |
| --- | --- |
| `python3 sweep11.py env` | record `env.json` (binary SHA-256s incl. the registered product-binary identity check vs plan9's `1b70b32f46d9218e`) |
| `python3 sweep11.py strata` | build `state/strata.json` (registered strata over the 660 open leaves of plan10's merged close; **committed before the ladder runs**) |
| `python3 sweep11.py smoke` | SMOKE: 8 leaves × 100k evals, full close/restore protocol (fresh phase + warm restore phase); not a datapoint |
| `python3 sweep11.py rung R1` | rung R1′: 256M child-evals per job on the 56 sampled leaves (fresh workers, no TT restore) |
| `python3 sweep11.py rung R2` | rung R2′: 1G per job on R1′ survivors (warm: `--tt-load` of R1′ dumps) |
| `python3 sweep11.py rung R3` | contingent rung R3′: 2G per job on R2′ survivors (warm) — fired only by the registered rule |
| `python3 sweep11.py r3` | contingent R3′ rule at the R2′ close: fired iff `elapsed + survivors × 2G / κ / R_camp ≤ 4.5 h` (κ = 27.5, R_camp = 558,529 — the locked plan8 constants); recorded in `state/r3_decision.json` either way |
| `python3 sweep11.py ladder` | R1′ → R2′ → R3′-decision, resumable per rung (R3′ never fires from the loop — only via the rule) |
| `python3 sweep11.py merge` | A2 merge pass: decisive results → fresh session seeded with plan10's merged master state → `campaign_master --resume --job-seed 11 --max-wall 60 ...` re-drain (verify failure = machinery defect, abort) |
| `python3 sweep11.py analyze` | M2 metrics + the pre-registered M2 gate + `state/ledger11.json` / `ledger11.csv` / `m2_corpus.csv` → `analysis.json` |
| `python3 sweep11.py status` | progress summary |

Rungs run strictly sequential, nothing else on the box, stdin from
DEVNULL. Session dirs under `/tmp`: `plan11_sweep`, `plan11_smoke`,
`plan11_merge`. TT dumps kept only for the latest rung, deleted at close.
Raw captures under `logs/` (gitignored); committed record = `sweep11.py`,
this README, `env.json`, `state/*.json` (strata, rung records, smoke, r3
decision, merge record, merged master state `master_state_merge11.json`,
`ledger11.json`, `analysis.json`) + the corpus CSVs.

## Registered shape

- Sampling frame: the 660 `Open` leaves of plan10's committed merged close
  (`../plan10/state/master_state_merge.json`; sanity-checked 728 leaves /
  68 Won / 660 Open / 27 children). The committed advisory of every open
  leaf is identity-checked against plan9's frozen S1 close at strata build
  (the re-drain never touches open leaves).
- Covariates: committed advisory `(pn, dn, work)` from plan9's frozen S1
  close + plan10's ledger cumulative spend. Touched = committed work > 0
  (104 leaves, committed dn ∈ [1113 … 146201]); untouched = 556.
- Strata (deterministic; committed as `state/strata.json` before the
  ladder): **H** = 24 (top committed-dn, ties → higher work, force-including
  the c1g5 near-misses `f7f6`, `g8f6`); **M** = 12 (touched, ~55th–75th pct
  of the touched set's committed dn); **L** = 12 (touched, ~15th–35th pct);
  **U** = 8 (untouched control, round-robin over children by static rank).
  Percentile bands pick evenly spaced ranks inside
  `[floor(p/100·(n−1)) … floor(p′/100·(n−1))]` of the touched set sorted by
  (dn asc, work asc, path). Primary corpus = H ∪ M ∪ L (48); U is analyzed
  separately and excluded from the primary gate.
- Ladder: R1′ = 256M (fresh), R2′ = 1G (warm from R1′ dumps), contingent
  R3′ = 2G (warm from R2′ dumps). Workers = 4, tt_mb 128, pt_mb 512,
  retention on, stable per-leaf worker affinity across rungs (leaf's index
  in the sorted open set mod 4), round-robin job naming `w{w}_{rung}_{n}`.
- SMOKE: 2 leaves per stratum × 100k evals, two phases (S0 fresh + S1 warm
  restore) exercising the full close/restore protocol; run in its own
  session dir, deleted at close, never merged.

## Registered analysis readings (fixed before any run)

- **Covariate regime matching (binding):** R1′ covariate = committed
  advisory dn from plan9's frozen S1 close (fresh regime; secondary). R2′/
  R3′ covariate = the prior rung's post-run advisory dn (the prior rung's
  job result `root_dn`; warm regime; **primary** — it matches the
  certificate consumption regime). Cumulative work is a registered control
  covariate: it must NOT separate (plan10 M3 AUC 0.000); if it does, the
  gate verdict is void until the defect is fixed and the affected rung is
  re-analyzed.
- Percentile rank: `pct(x, C) = (#c∈C: c<x + 0.5·#c∈C: c=x) / |C|`.
- Directional inversion (§4): a positive whose regime-matching pre-rung dn
  sits in stratum L's registered percentile range (or below it) —
  operationalized scale-free across regimes as
  `pct(pre_dn, rung entrant cohort) ≤ 0.35`.
- Pooled warm-regime AUC by rank-attachment: within-cohort percentile ranks
  pooled across warm rungs (R2′, R3′), AUC on pooled ranks.
- θ procedure (§4): θ = min pre-rung dn among R1′ positives if any (R2′/R3′
  held out); else θ = min pre-rung dn among R2′ positives with R3′ (if run)
  held out; with no held-out rung the analysis is in-sample-only (caveat in
  the report; a GO verdict under that branch additionally requires pooled
  AUC ≥ 0.85). Held-out false negative = held-out positive with pre-rung
  dn ≤ θ. S(θ) = fraction of R2′ primary entrants with pre-rung dn ≤ θ.
- Gate-band gap readings (registered): P ≥ 3 with warm AUC ≥ 0.80 and a
  clean direction but S(θ) < 0.40, and P ≥ 3 with warm AUC ∈ [0.60, 0.80)
  and ≤ 1 inversion / 0 held-out FNs, are **M2-MARGINAL** (the GO band's
  full conjunction is not met and NO-GO's disjunction is not fired).
- Any `Lost` result is a registered decided label and a machinery-noteworthy
  event (recorded per rung; positives = resolved leaves of either kind).
- The soundness contract of §3 of plan11.md is the second deliverable of
  this stage: it is carried **verbatim** into plan12 and amended into
  `campaign_architecture.md` §11 as sub-item A6b at the report.

## Binary identity

- Product binary must be byte-identical to plan9's record
  (`1b70b32f46d9218e…`); checked by `env` (reproduced with
  `CARGO_PROFILE_RELEASE_LTO=thin` — see `env.json`).
- Campaign binaries: sources git-identical to plan9's close commit
  `c6a30b0`; hashes differ from plan9's record (plan9's exact build context
  not reproducible from the commit — plan10 §9.2). Recorded in `env.json`;
  behavior risk carried by the A2 merge-pass verification.

## Budget (registered worst case)

R1′ 56 × 256M = 14.3 G; R2′ ≤ 55 × 1G = 55 G; R3′ ≤ 54 × 2G = 108 G →
≤ 178 G evals ≈ 6.5 G nodes (κ = 27.5) ≈ 3.2 h at the locked R_camp; the
contingent rule protects the 4 h working budget of the 6 h session cap.
