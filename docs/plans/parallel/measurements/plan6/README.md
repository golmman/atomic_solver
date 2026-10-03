# plan6 measurements (2026-10-04) — owner-directed re-measurement: length threshold, re-scoped gate

**Stage-3 verdict (plan6): NO-GO, re-registered gate failed on every gated
conjunct; condition-2 revert re-executed; initiative closed.** S4 = 1.01×
(m22) / 1.20× (rem12) / 1.65× (shuffle-win); I4 = 3.54× / 3.03× / 2.22×;
worst cap-hit rate 3/8 > 1/8; V = 0 (64/64 decisive `win`, zero panics).
The plan5b shuffle-win promise (2.68×/1.41×) was a 5-rep artifact of a
bimodal run distribution (5 healthy runs at 20–39 s vs 3 bad-tail runs at
90–100 s / 5.7–6.3× eval inflation per 8 t4 runs). Post-revert drift all
green (hashes identical to the plan4 record). Details: `report6.md`.

Executes `docs/plans/parallel/plan6.md`. Shared directory for the re-land
verification (stage 0), case selection (stage 1), W confirmation (stage 2),
and the campaign (stage 3). All runs on the release build, reference
container (Apple Silicon, 4 CPUs), default TT (128 MB), default ε (0.125),
default refine-cap (0.25). Code state: commit `b714ef6` content restored
verbatim + the plan5b W delta (`MAX_WORK_PER_JOB = 10_000` with its sweep doc
comment) — the exact state plan5b measured its campaign on.

## Stage 0 — re-land verification (all green, N = 1 surface)

| check | result |
| --- | --- |
| quick suite | 59/59 cases identical vs `baseline_quick_pre5.json` (`state/quick_reland.json`; fields status/outcome/nodes/child_evals/pv_len/timeout/wrong) |
| m22 stdout (30 s cap, FO, outcome-only) | sha256 `b7c74f17c88a730777be98a1452d97aa4cbccee49525cd47278b0a05b05ac79a` — identical to plan4/plan4b/plan5a records |
| shuffle-win stdout (100 s cap, FO, outcome-only) | sha256 `64129ef0942a0446faa4e062512bf80b6f34c2979f657f9f11f3853975b04cd7` — identical |
| snapshot dump | sha256 `eaa5f2b97bded5e1050cd2e1eb3489186985015d525035ea52cc4918cef81fde`, 195 B — identical |
| `make test` / `cargo fmt --check` / `cargo clippy --all-targets` | green / clean / clean |

## Stage 1 — case selection (locked before the campaign; no post-selection swaps)

Sequential candidate scan, first-outcome, product defaults, 60 s cap
(release build, this session, sequential runs only):

| candidate | wall | outcome | verdict |
| --- | --- | --- | --- |
| m21_black | cap-hit 60 s | draw (resource cut) | out |
| m22_black | cap-hit 60 s | draw (resource cut) | out |
| m23_black | 0.68 s | loss | too short |
| rem04 | 2.97 s | win | below window |
| rem10 | 4.52 s | win | below window |
| rem07 | 6.12 s | win | below window |
| rem08 | 6.69 s | win | below window |
| **rem13** | **11.05 s** | win | in window (backup) |
| **rem12** | **17.09 s** | win | **selected** |
| **rem11** | **19.60 s** | win | in window (backup) |
| rem09 | 26.85 s | win | in window (upper edge; not needed) |

Scan pool note (recorded per plan): the plan named the move-order fixture
(bimodal: m23_white 2.5 s / m20_white 100+ s), `tests/test_deep_outcomes.rs`
Win FENs, and the dec* conversion records (1.5–5 s class). The bimodality
gap was real for those pools; the window was filled from
`tests/fixtures/decisive_remaining.txt` (the repo's existing pool of
measured 10–130 M-eval Win cases — the natural home for the 10–30 s class;
its recorded child_evals estimates, measured at 64 MB TT, placed
rem04/07/08/10/12/11/09/13 near or inside the window). The four in-window
cases were all measured sequentially before selection.

**Selected mid case: rem12** — `rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10`,
white-to-move Win (same class as both gated endpoints), 17.09 s /
69,605,385 child_evals sequential (two identical driver runs — deterministic;
log-scale position ln(16.9/3.3)/ln(58.2/3.3) ≈ 0.57 between the endpoints).

### Locked case table (in-session sequential references, pre-campaign)

| case | role | FEN | cap | sequential wall | sequential child_evals | ref source |
| --- | --- | --- | --- | --- | --- | --- |
| m22 | short control (measurement only, no gate) | `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22` | 30 | 3.245 s | 14,156,269 | `benchmark --suite move-order --timeout 100` (in-session; evals identical to plan5b) |
| rem12 | mid, gated | `rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10` | 60 | 17.09 s | 69,605,385 | `measurements/plan6/seq_ref.rs` driver (2 identical runs) |
| shuffle-win | long, gated | `4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21` | 100 | 57.918 s | 249,480,478 | `benchmark --suite move-order --timeout 100` (in-session; evals identical to plan5b) |

`seq_ref.rs` (committed here) is the per-case equivalent of
`benchmark --suite move-order --json` for the non-suite rem12 case: it
constructs `Search::new(128)` with product defaults and prints
`name wall_s outcome nodes child_evals`. It is compiled as a temporary
`examples/seq_ref.rs` copy during the session and removed afterwards
(no repo example added).

## Stage 2 — W confirmation on the long class (then locked)

Pre-registered: shuffle-win t4, W ∈ {4000, 10000, 20000}, 3 reps each,
interleaved with N = 1 references. Keep 10 000 if it wins the median.
Driver: `sweep_w6.py` (plan5b's `sweep_w.py` adapted: same patch-and-rebuild
mechanism, shuffle-win case, 100 s cap, N = 1 reference interleaving).
Results: `state/sweep6_raw.jsonl`, `state/sweep6.csv`.

**Result: 10 000 does NOT win on the long class → W re-locked to 20 000**
(the pre-registered fallback). t4 medians: W=4000 → 29.44 s, W=10000 →
29.69 s (1 cap-hit), **W=20000 → 23.33 s (locked)**. N = 1 references
per block (58.7 / 57.8 / 57.8 s) confirm W never touches the sequential
path. Interpretation recorded in the locked constant's doc comment: W is
case-dependent (the plan5b m22 sweep punished 20 000 ~3×; the long case
rewards larger jobs), and the plan6 pre-registration locks the long-class
winner because the ship shape targets long solves. **Comparability
caveat (for the report): all plan6 campaign numbers use W = 20 000 while
plan5b's used W = 10 000 — the campaign is self-contained (its own N = 1
denominators), but plan5b vs plan6 cross-comparisons carry this confound.**

## Stage 3 — campaign (plan5b protocol, tightened)

Cases × N ∈ {1, 2, 3, 4}, ≥ 8 interleaved rounds per case (N = 1 = in-session
denominator). Metrics per (case, N): median wall + speedup (primary), work
inflation (median total evals / sequential evals), outcome agreement
(decisive runs must equal the sequential outcome), cap-hit rate (gate item
C ≤ 1/8), per-thread work split, panics/lock anomalies (zero expected).
N ∈ {8, 16}: log-fit extrapolations only, non-gating. Drivers:
`campaign6.py` → `state/campaign6_raw.jsonl`; `analyze_campaign6.py` →
`state/campaign6_summary.json`, `state/campaign6.csv`.

Logs under `logs/` are gitignored.

## Stage 3 — campaign results (medians over 8 interleaved reps; W = 20 000; in-session N = 1 denominators)

| case (seq ref) | N | median wall | speedup | inflation | cap-hits | agreement | helper share |
| --- | --- | --- | --- | --- | --- | --- | --- |
| m22 (3.263 s / 14.16 M) | 2 | 3.234 s | 1.01× | 1.98× | 0/8 | 8/8 win | 0.49 |
| m22 | 3 | 2.803 s | 1.16× | 2.55× | 0/8 | 8/8 win | 0.65 |
| m22 | 4 | 3.235 s | **1.01×** | **3.54×** | **1/8** | 7/7 win | 0.74 |
| rem12 (16.886 s / 69.61 M) | 2 | 12.701 s | 1.33× | 1.51× | 0/8 | 8/8 win | 0.49 |
| rem12 | 3 | 13.120 s | 1.29× | 2.31× | 0/8 | 8/8 win | 0.66 |
| rem12 | 4 | 14.070 s | **1.20×** | **3.03×** | 0/8 | 8/8 win | 0.74 |
| shuffle-win (57.707 s / 249.5 M) | 2 | 49.282 s | 1.17× | 1.76× | **3/8** | 5/5 win | 0.49 |
| shuffle-win | 3 | 33.755 s | 1.71× | 1.78× | 2/8 | 6/6 win | 0.65 |
| shuffle-win | 4 | 34.944 s | **1.65×** | **2.22×** | **2/8** | 6/6 win | 0.74 |

- Outcome agreement: every decisive run equals the sequential `win` (64/64);
  the 8 cap-hit runs are resource-cut `draw`s at the wall cap (rc = 0,
  documented timeout semantics). Zero panics, zero non-zero return codes.
- Length trend (H2): S4 = 1.01× (m22) → 1.20× (rem12) → 1.65× (shuffle-win),
  Spearman ρ = 1.0 over the three points — the direction H predicted is real,
  but the level collapsed at 8 reps (below).
- The shuffle-win t4 distribution is bimodal: 5 of 8 runs land 20–39 s
  (healthy: S ≈ 1.5–2.8×) and 3 of 8 land 90–100 s (bad-trajectory tail,
  total evals 1.4–1.6 G ≈ 5.7–6.3× sequential). The 5-rep plan5b median
  (21.7 s, S4 = 2.68×) sampled the good tail; the 8-rep median (34.9 s)
  does not survive it.
- N ∈ {8, 16} log-fit extrapolations (`state/campaign6_summary.json`,
  labeled non-gating): unreliable here (non-monotone medians, S4 < S3 on
  shuffle-win) — reported only for completeness, never as wall claims.
