# Report 6 — the PNS selection mechanism: eligibility, pacing, rationing; configured and A/B'd

Executes `plan6.md`. All deliverables landed; all pre-registered gates
**pass**; the four-arm A/B completed from the identical post-batch-2 state.
**The verdict: the ladder is a measured no-go** — the §4.5 decision rule
fired on the first branch (B ≤ C on facts, all 9 rungs censored, no depth
dividend), so the default config is **arm C's** (`reserve_share = 0.0`):
the mechanism ships as **rationed expansion only**, with the rung machinery
(eligibility trigger, pacing, growth, rotation) kept config-reachable for
later batches. This closes the question plan5 opened. The plan's
documentation-first rule held: docs + `examples/`-side code only; the
product solver (`src/`), DB schema, and spec are untouched; no in-session
re-propagation (plan4's simplification stands).

## Deliverables

- **D1 — mechanism + config** (all ≤ 10 KiB; split as planned):
  - `pns/config.rs` (+ `pns/config/tests.rs`) — the `--pns-config` TOML
    surface: the seven knobs with compiled defaults, unknown keys rejected,
    validation (`reserve_share ∈ [0,1]`, `interleave_k ≥ 1`), `describe()`
    echo, and the named presets `degenerate()` (arm A′),
    `mechanism_defaults()` (arm B, the §2 table verbatim),
    `ladder_deleted()` (arm C — now identical to the compiled default).
  - `pns/selector/decision.rs` (+ `rung.rs`, tests in `decision/tests.rs` +
    `decision/tests_pacing.rs`) — the pre-registered decision table (§2):
    reserve check → interleave slot (`V ≡ 0 mod k`) → fallback drain →
    expand; the eligibility derivation (`rung-eligible = censored and no
    virgin child`, derived from ledger + queue state on every visit; the
    child-visit test handles rows, ledger records, proven rows, and
    mid-session decisions) and the rotation (`fewest-passes` |
    `number-ply-path`).
  - `pns/pacing.rs` — the `Pacing` state: config, session cap, `V`,
    reserve charge (actual child-evals of completed rung visits), per-ply
    layer visit counters, per-node rung counters.
  - `pns/selector.rs` — `VisitKind`/`Selection.kind`, `pop_capped`
    (the plan4 `pop()` verbatim at cap 0 — the equivalence contract's
    core), and one correctness fix: `on_censored` now re-classifies the
    exposed children's slots in the parent's children vec
    (`Unvisited → LedgerNode`) — number-equivalent (a fresh record is 1 +
    0 = 1 = unvisited) but required so eligibility derivation sees the
    true child state.
  - `pns/driver.rs` (moved from `batch.rs`), `pns/census.rs` (moved from
    `pns.rs`), `session/record.rs` (JobRecord moved), `harvest_args.rs`
    (CLI args moved) — module splits for the file-size convention; the
    census `kind` field on `job:` lines (legacy policies emit without it),
    the rung count on the harvest summary line, and the `cfg` echo on the
    session-start `pns:` line.
  - Tests (`tests/proofdb.rs` target): the decision-table suite —
    degenerate-config ≡ plan4-selector sequence equality (60 censored
    visits, byte-identical ledger), eligibility edges + the
    last-virgin-child transition, reserve exhaustion routing,
    `max_rung_passes`, rotation disagreement, fallback drain + exhaustion,
    layer-cap rationing, `eligibility = "always"`, linear/constant growth,
    config TOML validation, census/config echo. Product tests untouched
    (240 lib tests, all green).
- **D2 — the A/B** under `measurements/plan6/`: `census_arm_{A,A_ap,B,C}.json`
  (job records + summaries + session-start census), `verdicts.json` (gate
  verdicts, arm-B mechanics, the §4.5 decision table), the four per-arm
  post-run ledger snapshots, `env.json`, `README.md` (provenance, command
  table, verdict table), and the `assemble_census.py` driver.
- **D3 — `research_pns_ladder.md`**: the provenance chain
  (report3 pivot → report4 finding 3 → plan5 lemmas → the design dialogue →
  the mechanism), the theory linkage (PN-search breadth explosion from the
  vendored `deep-dfpn-2017`/`pdfpn-2010` extractions; df-pn threshold
  escalation as the ladder's ancestor; the eligibility trigger identified
  as the original part), the rejected alternatives, and the vendoring
  attempt for Nagai's thesis (not retrievable — Wayback/PPoW/search
  blocks; entry stays **Cited**, linkage mined from the vendored
  secondary sources). `docs/bibliography.md` updated (Allis, Nagai,
  Zhang 2017 mining pointers).
- **D4 — this report.**

## The A/B (root = startpos; staging copies; cap 300M, base 4M; no mid-run tuning)

| arm | config | jobs | kind split | facts | plies visited (jobs) | evals | post-run ledger |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A — status-quo control | pre-refactor binary, plan4 selector | 75 | 75 expand | 0 | 2 (75) | 300,000,926 | `7fd92ea2…` (4,933 records) |
| A′ — equivalence replay | degenerate (`reserve 0, layer cap 0`) | 75 | 75 expand | 0 | 2 (75) | 300,000,926 | `7fd92ea2…` — **byte-identical to A** |
| B — mechanism defaults | §2 table verbatim | 64 | 55 expand + **9 rung** | 0 | 0 (2), 1 (7), 2 (24), 3 (24), 4 (7) | 300,000,785 | `9aa0c326…` (4,178 records) |
| C — ladder deletion | `reserve 0, layer cap 24` | 75 | 75 expand | 0 | 2 (24), 3 (24), 4 (24), 5 (3) | 300,000,925 | `058a6202…` (4,569 records) |

Session-start census (all arms, modulo the `cfg` echo): `pns: rows 35055
(open 75), ledger records 3024 (3024 new, 0 dropped as decided);
exclusions proven-ancestor r26/l0, implied-win r0/l0, implied-loss r0/l0;
jobs 3073 (rows 49, ledger 3024); base budget 4000000` — matching plan5's
derived arithmetic (2,923 number-1 + 150 number-2) exactly.

**Arm B's mechanism trace (the D1 census field's first payoff):** rungs at
V = 0, 4, 8, 12, 16, 20, 24, 28, 32 → the root pass 2 (8M), then a2a3,
a2a4, b1a3, b1c3, b2b3, b2b4 (eligible at session start — their ply-2
children were fully censored by batches 1–2, verified from the plan5
ledger), then c2c3 (its 10 virgin children closed by this session's
ply-2 visits — the eligibility trigger measured flipping mid-session),
then the root pass 3 (16M). Rotation `fewest-passes` → ties by
`(ply, path)` produced exactly this order; rung budgets 8M (7×) + 16M +
8M = 80.0M ≥ the 75M reserve → **reserve-bound**, as pre-registered.
All 9 rungs censored.

## The §4.5 decision rule — the ladder's marginal value

| metric | B | C | reading |
| --- | --- | --- | --- |
| facts | 0 | 0 | B ≤ C ✓ (rule's first branch) |
| max ply reached | 4 | **5** | no depth dividend — C reached deeper |
| evals per ply layer | 0: 24.0M, 1: 56.0M, 2–3: 96.0M, 4: 28.0M | 2–3: 96.0M, 4: 96.0M, 5: 12.0M | B spent 80.0M on plies 0–1 revisits that settled nothing |
| rung yield | 9 rungs, 0 facts, all censored | 0 | no dividend |

**Verdict: ladder no-go.** The default config becomes arm C's: the
compiled default `reserve_share` is now **0.0** (documented in the
`Default` impl; `mechanism_defaults()` keeps the pre-registered arm-B
preset reachable by name, and `arm_b.toml` in the run commands reproduces
it verbatim). The mechanism ships as rationed expansion only; the rung
machinery stays exposed for later batches (`eligibility`, `rung_growth`,
`interleave_k`, `max_rung_passes`, `rotation` all live).

## Standing-state effects

- **0 facts in every arm** (as pre-registered: yield ≈ 0) → promotion
  vacuous: no shard files written, the standing manifest and shard set
  untouched, the DB unchanged (`670e19e3…` confirmed after all arms).
- **The standing `data/proofdb_work.json` advanced to the winning arm's
  post-run ledger** (arm C: sha `058a6202…`, 4,569 records = 225 censored
  [root + 20 ply-1 + 153 ply-2 + 24 ply-3 + 24 ply-4 + 3 ply-5] + 4,344
  fresh). See finding 4 for what this drops.
- Same-path outcome contradiction between arms: none possible (0 facts);
  the soundness tripwire stayed clear.

## Gate verdicts

| gate | verdict | notes |
| --- | --- | --- |
| H1 job/shard integrity | **pass** | per arm: the census line (pinned post-batch-2 values, modulo `cfg`), 0 lineage-gate drops, exclusion census per reason; every job path present in its arm's post-run ledger; the census quadruple consistent (`number == pass`, fresh jobs at (1, 1, 0), revisits at cumulative-4M work) — all checked in `assemble_census.py` |
| H2 merge determinism | **pass** | merger re-run twice over the standing manifest/shards → DB `670e19e3…` and dump `e3060d14…` byte-identical both times and to the input DB (0 promoted facts → the censored-session residue check) |
| H3/H4 no-regression / spot-checks | **pass (vacuous)** | 0 new facts, 0 shards |
| H5 policy determinism | **pass** | per-arm replays (A′, B, C + arm A with the pre-refactor binary): job sequences identical excl `wall_s`, post-run ledgers byte-identical; **plus the A′ ≡ A equivalence match**: 75/75 records identical on every field except `wall_s` and `kind`, ledgers byte-identical (`7fd92ea2…` both) — the new selector verified as a strict superset of the plan4 selector at full-batch scale |
| H6 hygiene | **pass** | `make test` green (incl. the 14 new decision/config tests; 46 in the proofdb target), `cargo clippy --release --all-targets` 0 warnings, `cargo fmt --check` clean, `cargo doc` clean; no `src/` changes (`git status`); new/edited example files ≤ 10 KiB; `data/` ignored |

Compute: 8 harvest runs (4 arms + replays) ≈ 9 min + merger runs ≈ 2 min +
build/test cycles — within the §8 budget.

## Findings

1. **Arm B's reach prediction was wrong (measured ply 4, predicted ≈ 8).**
   The prediction assumed the ply-4+ number-1 pool stays sparse (the
   unvisited rows); it ignored that each ply-3 censor exposes ~21 fresh
   ply-4 records (~500 after ply 3's 24 visits), so the post-ply-3
   expansion went into the ply-4 fresh pool (7 visits) and stopped at the
   eval cap. Lesson recorded in the measurements README: per-layer visit
   caps ration *breadth within a layer*; they do not by themselves force
   depth progress because the fresh-exposure cascade re-seeds the next
   layer at number 1. A depth-seeking policy needs a different lever
   (e.g. a cap on fresh exposures per layer, or a ply-front bias) — a
   candidate for a future plan; not tuned here (§4.6's no-mid-run-tuning
   rule held).
2. **The eligibility trigger works, is not vacuous, and is not the
   binding constraint.** Eight nodes were eligible at session start (root
   + seven ply-1 nodes whose children batches 1–2 had fully censored —
   verified from the plan5 ledger before the run) and one flipped
   mid-session (c2c3). The reserve was the binding limit (9 rungs /
   80.0M evals; the final rung overshot the 75M reserve by one budget,
   per the caps-not-quotas rule). A future ladder experiment should tune
   `reserve_share`, not `max_rung_passes`.
3. **The interleave reading is pinned by implementation:** `V` counts
   previously executed visits and the rung slot is `V ≡ 0 (mod k)` — the
   first visit of a session is a rung slot whenever something is eligible
   and reserve remains. Documented in `decision.rs`; the census `kind`
   trace makes the schedule reproducible from any transcript.
4. **The standing-ledger advance drops arm A's censor knowledge** (the 75
   additional ply-2 nodes A/A′ censored are back at `passes 0` in C's
   ledger). This is pre-registered (§4) and self-consistent (the winning
   arm's state is the state the next session continues from), but it
   means the next session re-censors ~75 known-quiet ply-2 nodes at 4M
   instead of resuming them at pass 2. Worth remembering in batch 3's
   sizing.
5. **Nagai's thesis stays unvendored** — no retrievable copy found
   (Wayback snapshots of the author site carry no thesis PDF;
   scholar.archive.org and search engines challenge-block scripted
   access). The threshold-escalation linkage is mined from the vendored
   `deep-dfpn-2017` / `pdfpn-2010` extractions, which state the df-pn
   mechanics directly. A manual-download retry is optional.

## Problems encountered

- **A pop-sentinel defect caught by the new tests before any run:**
  `pop_capped`'s "cap 0 = unlimited" mapping was lost in the
  u64→usize conversion, silently disabling all selection (found via the
  decision-table suite, not by the arms); fixed and covered.
- **The degenerate-equivalence test initially failed at visit 24** — the
  reserve shortcut (`session_cap == 0 → unlimited`) outranked
  `reserve_share = 0` in the test build, firing rungs under the
  degenerate config. Fixed by making `reserve_share = 0` mean no rungs
  regardless of the cap (the equivalence contract's precedence), and the
  full-batch A′ ≡ A byte-match confirms it.
- **The eligibility test fixtures needed the OR/AND number coupling
  accounted for** (an OR parent's number is always ≤ its child's number,
  so the rotation-disagreement test required two ledger nodes, not the
  root-child pair); documented in the test.
- The 10 KiB file-size convention required five module splits
  (`config`, `census`, `pacing`, `rung`, `driver`, `record`,
  `harvest_args`) — pure code motion, no behavior change beyond D1.

## Unresolved / next steps

1. **Batch 3 under the new default** (rationed expansion, layer caps 24,
   reserve 0): the standing ledger holds 4,344 fresh records (3,165 at
   ply 3, 469 at ply 4, 456 at ply 5, 190 at ply 2, 64 at ply 6) + 225
   censored at pass 1–4. A session sizing note should account for
   finding 4 (the ~75 re-censors) and finding 1 (caps ration breadth,
   not depth).
2. The depth-rationing lever (fresh-exposure cap or ply-front bias) is
   the natural next selection-mechanism question if the project wants
   deeper reached plies per session; it needs its own pre-registered
   plan.
3. Items 3 (DTM-upgrade pass), 5 (website handoff), 6 (parallel
   harvesters), 7 (subtree scoping) unchanged.

## Tools used

`cargo build/test/clippy/fmt/doc` (release), `python3` (census assembly +
ledger/census probes + a tiny Wayback/CDX vendoring probe), the
merger/harvest binaries, `git` (read-only). All runs against throwaway
staging copies in `/tmp/plan6/`; the standing `data/` layer was touched
only by the pre-registered ledger advance; the transcripts live in
`/tmp/plan6/` (regenerable per the README command table; the parsed
census is the record).

SESSION COMPLETE
- `report6.md` written; `measurements/plan6/` complete (4 arm censuses,
  verdicts.json, 4 ledger snapshots, env.json, README, assemble driver);
  `research_pns_ladder.md` + bibliography updated (D3); gate result: H1–H6
  all pass, A′ ≡ A byte-match confirmed, 0 facts in every arm as
  pre-registered; **the ladder measured a no-go** — default config is arm
  C's (compiled `reserve_share = 0.0`), mechanism ships as rationed
  expansion only.
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan7.md: batch 3 under the
   new default (rationed expansion, layer caps 24) over the standing
   post-plan6 ledger, with a pre-registered sizing note covering
   report6's findings 1 and 4." — the natural continuation; batch 3 needs
   a plan first (the plan6 session does not draft it).
2. Alternative: draft the depth-rationing lever plan (finding 1) before
   batch 3 — safer only if the project wants deeper reached plies per
   session; otherwise batch 3 measures the default policy as shipped.
