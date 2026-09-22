# Report 2: Solved-Set Frontier Push — substrate gate verdict NO-GO

Initiative: `solve`. Executes `plan2.md` (SSFP mechanism spec,
pre-registered M1 transposition-substrate gate, 2 h pilot). Runs of
2026-09-22. Harness: `measurements/plan2/ssfp.py` + command table in
`measurements/plan2/README.md`. This session ran on the reference
sandbox (4-core quota `cpu.max 400000/100000`, 8 GiB cgroup memory
limit, ~15 GB RAM visible), release build of the unchanged plan1 source
(`src/` last touched at `6c513fe`, the post-revert state plan1's drift
captures pin).

## TL;DR

- The **M1 substrate gate fires NO-GO** (pre-registered: GO ≥ 5%,
  NO-GO < 1%). Median directed value share over 1,438 cross-system
  snapshot pairs: **0.0000%** — not a near miss; the distribution is
  zero-inflated (1,074 of 1,440 pairs share literally nothing).
- The deepest finding is stronger than the gate: **even same-system,
  ±2-ply snapshot pairs share ~0%** (d4_p9→d4_p11: 0.08%). The only
  substantial reuse (95.9%) is the degenerate pair where two ladders
  converged to the *identical position* — i.e. the same search twice.
- Conclusion: **the censored runs' TT solved sections (~1.5 M records
  each, ~35% of visited keys) are search-local, not a reusable
  substrate.** A solved set S grown from quiet-run deposits does not
  cut other quiet runs' work — and by the secondary metric it would
  have covered at most 10.75% of even its own family's tactical proof
  tree (p30).
- Per the plan's gate semantics, **no §3 product surface landed** (the
  `--tt-load-path` / `--frontier-dump` implementation staged during the
  regeneration window was reverted after the verdict; see §6) and **the
  pilot loop did not run**. The initiative rethinks again; §9 frames
  the two pre-registered options with what the data says about each.

## 1. What was run

1. **Harness** (plan task 1): `measurements/plan2/ssfp.py` (env /
   regen / m1 / status; strict binary readers for TT-snapshot v1 and,
   via `examples/pt_keys`, replay-derived proof-tree node keys),
   `sstore.py` (S store merge/export v0 + selftest), `pilot.py` (Arm A
   / Arm B / substrate drivers — **unexercised**, gated by the verdict).
2. **Snapshot regeneration** (task 2): the 48 censored plan1 cost runs
   replayed sequentially, exactly the plan1 commands
   (`--timeout 120 --first-outcome --tt-dump-path`), snapshots **kept**
   this time. 48/48 censored at the cap, wall 122.8–123.3 s each,
   **98.4 min total** (plan predicted ~107 min). Fidelity: nodes =
   0.94–1.17× plan1's (median 1.06×; wall-clock-bounded runs are
   content-equivalent, not byte-identical — as recorded in the plan).
   Per snapshot: solved 1.14–1.89 M (median 1.55 M), unsolved
   2.30–3.04 M (median 2.63 M).
3. **M1 computation** (`ssfp.py m1`): strict readers parse the 48
   snapshots; directed value share
   `vshare(A→B) = |A.solved ∩ (B.solved ∪ B.unsolved)| / |A.solved|`
   over all ordered snapshot pairs from different pre-registered
   systems with ≥ 1,000 solved records (`m1.json`). ~10 min.

## 2. What M1 measured (and two data-quality findings)

**Finding A — the plan's system list over-covers the censored data.**
The 48 censored runs belong to 5 lines: e4 (10), d4 (10), nf3 (10),
d4d5 (14), startpos (5). The tactical families named as M1 systems
(e4e5, e4c5, nf3d5) have **zero** censored runs — plan1 measured every
one of their positions as cheap uncensored proofs. startpos is not a
plan-listed system (excluded from M1; included in the secondary
metric). So the metric is effectively over **4 systems**
{e4, d4, nf3, d4d5} → 1,440 ordered pairs after the ≥1,000-solved
filter (all 48 snapshots qualify).

**Finding B — one degenerate pair.** d4_p21 and e4_p21 have the
*identical root FEN* (`rnbqkbnr/2pppppp/8/8/P1PPP3/5PPP/8/2BQKBNR b
Kkq - 0 11`): the two self-play ladders converged. Their pair
(95.89% / 93.37% directed) is the same search run twice, not
transposition substrate. Excluding it: 1,438 pairs.

## 3. Gate verdict: NO-GO (pre-registered)

| group (unordered) | n | median | max | %nonzero |
| --- | --- | --- | --- | --- |
| d4/d4d5 | 280 | 0.00% | 16.70% | 34% |
| d4/e4 | 200 | 0.00% | 95.89% (degenerate) | 37% |
| d4/nf3 | 200 | 0.00% | 1.22% | 21% |
| d4d5/e4 | 280 | 0.00% | 0.98% | 26% |
| d4d5/nf3 | 280 | 0.00% | 0.31% | 14% |
| e4/nf3 | 200 | 0.00% | 0.29% | 21% |
| **all pairs** | **1,440** | **0.0000%** | 95.89% | 25% |

**M1 = 0.0000% < 1% → NO-GO.** 1,074 of 1,440 pairs share literally
zero keys. The verdict is not marginal and not sensitive to the
degenerate pair or to the threshold.

The strongest sanity check: **same-system adjacent-ply pairs are
equally empty** (d4_p9→d4_p11 0.08%, d4_p11→d4_p13 0.43%,
d4d5_p2→d4d5_p4 0.22%, d4d5_p4→d4d5_p28 0.00%). A broken reader or a
key-encoding bug would not produce that pattern alongside a ~96%
same-position pair; the metric implementation was additionally
cross-checked with two independent intersection methods (set-based and
sorted-array based).

## 4. Interpretation: why the substrate is empty

The gate's question was whether quiet defense systems' searches
traverse regions that other systems' cheap proofs resolved. The answer
is no, and the ±2-py result sharpens it into a mechanism-level
statement:

- A 120 s censored search proves ~1.5 M interior nodes (~35% of the
  ~4.4 M keys it touches). Those *solved* records concentrate near that
  search's own proof frontier. Two searches whose roots differ by even
  one quiet reply explore **disjoint neighborhoods** of the augmented
  DAG: their (position, halfmove-clock) key sets overlap only where
  forcing lines coincide, which for quiet roots is ~never.
- This is consistent with plan1's bimodality finding and sharpens it:
  the quiet-defense censoring is not "the same hard region reached from
  many roots" — each root has its own hard region. Work spent proving
  one quiet system's interior is a dead cost for every other root.
- Consequently the SSFP bet — *cheap proofs deposit values in regions
  later quiet-position proofs traverse* — is measured empty at this
  scale, in both the cross-system form (M1) and the adjacent-ply
  within-system form. The pilot's Arm B substrate metric (re-solve the
  same root with S preloaded) would only ever have exercised the
  degenerate same-root case, where reuse is trivially ~96% (the d4_p21
  / e4_p21 pair is a direct measurement of exactly that ceiling).

## 5. Secondary metric (non-gating): proof-tree key coverage

| tree | keys (unique) | median coverage | max coverage |
| --- | --- | --- | --- |
| reconstructed_d4d5_p30.bin (32,942 nodes) | 13,078 | 0.00% | **10.75%** (d4d5_p28) |
| reconstructed_d4d5_p32.bin (68 nodes) | 38 | 0.00% | **78.95%** (d4d5_p28) |

Even within the d4d5 family, the deepest regenerated quiet snapshot
(p28; the p30/p32 *own* snapshots were uncensored-run artifacts, pruned
in plan1 and not part of the 48) covers ~11% of the p30 tree's keys.
The tiny p32 tree (38 keys, sits directly on the ladder line) is mostly
covered by its own family — as expected for a 1-ply-different root.
Read jointly with §4: the tactical region does not anchor quiet
searches, and quiet searches do not anchor even their own family's
tactical proofs.

(Side observation, recorded for the proof-tree layer: the p30 dump
contains 32,942 nodes but only 13,078 unique keys; 884 keys appear at
two graph depths with parity-derived outcomes flipped. That is a
property of the finalized tree's transposition nodes, not a validator
defect — plan1's `validate: ok` stands — but a future artifact
composition step must key by position, not by node id.)

## 6. Problems encountered

1. **First M1 pass OOM-killed** (8 GiB cgroup): holding 48 solved-key
   Python sets (~80 MB each) exceeded the limit. Fixed with a
   memory-lean pass: per-snapshot sorted `array('Q')` (~12 MB each)
   resident + one transient all-keys set; two independent intersection
   methods agree.
2. **Stale `pt_keys` binary**: the 6-field output format was edited
   during the regeneration window but could only be built after it
   (builds were suppressed while censored runs were in flight — they
   are wall-clock-bounded and CPU contention would distort them). The
   first post-regen build then surfaced two real compile errors in the
   staged frontier code, which is when the §3 code was still staged.
3. **§3 implemented before the verdict, then reverted.** To overlap
   the ~98 min regeneration wall with development, the §3 product
   surface (`--tt-load-path`, `--frontier-dump`, `--frontier-count`,
   `search::dfpn::frontier`, CLI tests, AGENTS.md option list) was
   fully implemented ahead of the gate. The gate then fired NO-GO, and
   per the plan ("no product changes before this verdict", task 3 "on
   GO") it was **reverted completely**; the fast test gate is green
   (347 passed / 0 failed) on the reverted tree. Residual work product:
   this report and the measurement tooling below.

## 7. Deviations from the plan

- **Kept (measurement tooling, not gated surface):**
  `examples/pt_keys.rs` — required to reproduce `m1.json`'s secondary
  metric (the binary proof-tree dump stores no hashes; keys are
  recomputed by replay, mirroring the validator). Documented in
  AGENTS.md's examples list. It also emits parent ids and move bits so
  a future store merge can derive Win best moves.
- **Kept but unexercised:** `sstore.py` (S store v0 merge/export +
  selftest) and `pilot.py` (Arm A/B/substrate drivers). These are §3.3
  campaign tooling that was gated on GO; they are archived in
  `measurements/plan2/` (selftest-verified, never run against real
  data) because they document the v0 mechanism concretely for the
  rethink. Marked as unexercised in the README.
- **Tooling location**: plan §3.3 said `examples/` or a `campaign/`
  dir; the Python campaign tooling lives in `measurements/plan2/`
  (stdlib-only, following the plan1 harness layout; `examples/` stays
  Rust-only). A top-level `campaign/` dir is deferred to whenever a
  campaign prototype actually exists.
- **M1 metric implementation choice** (pre-data, documented in
  `ssfp.py`): pairs are snapshot-level per the plan; the 1,440-pair
  population follows from the plan's definitions once Finding A is
  accounted for (the plan's 7 named systems include 3 with no censored
  data). startpos snapshots excluded from M1 per the plan's system
  list, included in the secondary metric.

## 8. Missing tests

- `pilot.py` and the `sstore.py` export path were never exercised
  against real solver output (gated off); the selftest covers store
  round-trip, class separation, and export format validity, but not
  the queue loop's budget accounting or frontier parsing on real
  `--frontier-dump` output (the Rust frontier walk itself never got to
  run at all before revert).
- The 884 duplicate-key nodes in the p30 tree (§5 side observation)
  deserve a proof-tree layer test (transposition nodes under
  parity-derived outcomes) — not written; observation only.

## 9. Next steps (decision pending — the initiative rethinks again)

The plan pre-registered two NO-GO branches; the data above informs
both:

1. **Close with the artifact.** The repo's verifiable-artifact backbone
   (search → snapshot → reconstruct → replay-validated dump) is done
   and exercised; what does not exist is a *campaign mechanism* that
   makes the startpos reachable. This branch would record the SSFP
   NO-GO as the campaign's terminal measurement: the structural-floor
   record (`research/structural_floor.md`) plus plan1's bimodality plus
   plan2's empty-substrate result jointly say the startpos value is not
   reachable by any measured mechanism on sandbox-scale resources.
2. **Sharpness-first rescoping.** The only measured-positive signal in
   this whole initiative is plan1's tactical class: several opening
   mainlines are *forced wins ≤ 9 plies*, proven in ≤ 5k nodes. A
   rescope that drops the startpos-value goal in favor of exhaustively
   proving (and artifact-ing) the full tactical-win class — e.g. all
   ≤9-ply forced wins reachable within a fixed opening book — needs no
   new substrate mechanism (no S, no anchors, no frontier queue); it is
   a bounded enumeration over the cheap class, verifiable end-to-end
   with the existing pipeline. Everything measured so far (plan1 cost
   data, plan2's empty substrate) is consistent with the quiet class
   being the only hard part and being architecturally separated from
   the tactical class.

My recommendation, as the consultant on record: **option 2**, because
it produces a real, verifiable artifact soon and leaves the quiet-class
question to a future mechanism innovation (the Čížek job-level
architecture, or a GHI-correct clock-aware anchor layer) instead of
burning sandbox-months on a substrate this plan has now measured empty.
But the choice — and whether the initiative closes or rescopes — is the
user's pivot decision, as in plan1.
