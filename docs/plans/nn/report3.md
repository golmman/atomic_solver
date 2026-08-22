# Report 3: subtree-size proxy ablation (design A, Gate 0.5)

## Summary

Plan `docs/plans/nn/plan3.md` (design A) is implemented and measured:
`Search::tt_work_for` plus `examples/work_proxy_ablation.rs`, the integration
tests, and the `AGENTS.md` entry. The ablation answers the open question from
`report1.md` / `report2.md` — **does the corpus's `subtree_size` label proxy the
solver's real per-child work?** — and the answer is **no**: the proxy is not
trustworthy as an AND-node ranking label.

Measured on the corpus settings (`--timeout 10`, 64 MB TT, 256 MB proof tree),
with near-complete TT coverage (~99.6%):

| Metric | quick (36 cases) | decisive (23 cases) |
|---|---|---|
| AND nodes (Loss, ≥ 2 children) | 22,940 | 18,751 |
| complete nodes (all children probed) | 22,694 | 18,515 |
| coverage (children probed / total) | 99.7% | 99.6% |
| child pairs | 165,479 | 147,603 |
| **pair flip rate** | **28.0%** | **28.7%** |
| Kendall τ (pooled over all pairs) | 0.60 | 0.59 |
| Kendall τ (mean over complete nodes) | 0.335 | 0.347 |
| top-child agreement | 83.3% | 81.8% |
| **work-weighted flip share** | **47.7%** | **45.0%** |

The pair flip rate (28–29%) and the work-weighted flip share (45–48%) are
three to five times the plan's tentative go/no-go ceiling (≤ 5–10%). **Verdict:
escalate to design B** — real per-child `work` counters in `NodeProven` /
`ProofNode`, a dump v2, and a corpus v2 whose AND label ranks children by real
work. Gate 2 (`plan_external_trainer.md`) should not train on `subtree_size`
rankings as-is.

## CLI as implemented

```
work_proxy_ablation [OPTIONS]
  --fen <FEN>          single position; case name "fen"
  --suite <NAME>       quick | decisive | all   (default: quick)
  --timeout <S>        search budget in seconds  (default: 10)
  --epsilon <F>        DF-PN+ threshold          (default: 0.125)
  --tt-size <MB>       TT size                   (default: 64)
  --pt-size <MB>       Proof-tree memory budget  (default: 256)
  -h, --help           exit 0; unknown option exit 1
```

Suite mapping: `quick` = decisive + move-order cases `m ≥ 23` (identical to
`corpus_gen --suite quick`); `decisive` = the decisive fixture alone;
`all` = move-order + decisive. `--fen` bypasses suites.

Per case: `Position::from_fen` → `Search::new(tt_size)` with
`set_timeout`/`set_epsilon` → `ProofTreeWorkerHandle::spawn` +
`search.set_proof_event_sender` → `solve_with_progress` → on `Draw` outcome
synthesize a Loss root (the same workaround as `corpus_gen` /
`move_order_fractions`) → `handle.finalize()` → `handle.tree()`. The `Search`
(and its TT) is **kept alive** through the tree walk so child hashes can be
probed, then dropped.

The tree is walked with a single mutable `Position` (the report2 replay
invariant, so a malformed dump still crashes loudly instead of desyncing). At
every node with `outcome == Loss` and ≥ 2 children it reads each child's
`subtree_size` (post-order) and probes `search.tt_work_for(child.hash)`. A node
is *complete* when every child hash hit the TT; only complete nodes contribute
pair metrics.

Output schema:

- **stderr** (per case, two lines, plus progress):
  ```
  === dec10  outcome=win  and_nodes=16927  complete=16713  coverage=99.6%  timeout=yes
      pairs=98187  pair_flip=32.7%  kendall=0.53  kendall_mean=0.32  top_agree=80.5%  work_flip=40.9%
  ```
  `timeout=yes` / `memory_limited=yes` flags are appended like
  `move_order_fractions`.
- **stdout** (final summary table only): one header line, one row per case,
  and a final `aggregate` row with the same columns.

Metric definitions (documented in the example header):

1. **Pair flip rate** — over all unordered child pairs of complete AND nodes,
   the fraction where `sign(subtree_size_i − subtree_size_j) !=
   sign(work_i − work_j)`. Tie-vs-strict mismatches count as flips in both
   directions (symmetric), so this is the conservative "label noise the
   trainer would see".
2. **Kendall τ** — `(C − D) / (C + D)` over pairs strictly ordered in both
   dimensions, reported pooled over all pairs *and* as the mean over complete
   nodes.
3. **Top-child agreement** — the fraction of complete AND nodes where a child
   attains both the max `subtree_size` and the max `work`.
4. **Work-weighted flip share** — flipped pairs weighted by
   `min(work_i, work_j)` divided by the total `min` weight over all
   complete-node pairs.

## Accessor and unit test

`src/search/dfpn/mod.rs`:

```rust
/// Real work recorded in the transposition table for a position hash.
///
/// The value is the cumulative `child_evals` spent under that subtree.
/// `None` if the entry was evicted or never stored.
#[must_use]
pub fn tt_work_for(&self, key: u64) -> Option<u64> {
    self.tt.probe_summary(key).map(|s| s.work)
}
```

Unit test `tt_work_for_returns_stored_work` in `src/search/dfpn/tests.rs`
stores an entry via `search.tt.store(...)` and asserts the probe returns the
stored `work` and `None` for an absent key. No other `src/` change (design A
as pinned: no dump-format, worker, scorer, or corpus change).

## Measured numbers per suite

Full per-case table (quick run, `--timeout 10`, `--tt-size 64`, `--pt-size
256`, release):

```
case        and  comp   cov%     pairs  flip%  kendall  kmn%  top%  workflip%
dec01         418  408   99.6%    19221   21.1%     0.84   57.4%   97.5%      29.3%
dec02         277  277  100.0%      683   24.2%     0.73   65.8%   89.2%      28.2%
dec03           2    2  100.0%      570    0.0%     1.00  100.0%  100.0%       0.0%
dec04          24   24  100.0%      100   35.0%     0.76   34.2%   91.7%      69.5%
dec05          75   75  100.0%      591    8.8%     0.86   36.9%   92.0%      49.9%
dec06         204  204  100.0%     1187   22.3%     0.73   34.9%   91.2%      51.2%
dec07           3    3  100.0%       13   15.4%     1.00   66.7%  100.0%      28.8%
dec08           0    0    0.0%        0    0.0%     0.00    0.0%    0.0%       0.0%
dec09           1    1  100.0%        1    0.0%     0.00    0.0%  100.0%       0.0%
dec10        16927 16713   99.6%    98187   32.7%     0.53   32.5%   80.5%      40.8%
dec11          22   22  100.0%     2527    7.7%     0.84   12.4%   95.5%      61.8%
dec12          33   33  100.0%     2693   32.5%     0.55   72.0%   81.8%      61.2%
dec13         156  146   98.4%     2461   22.6%     0.88   41.2%   98.6%      48.4%
dec14         156  155   99.9%     3482   27.2%     0.85   42.4%   93.5%      77.5%
dec15         173  173  100.0%      977   12.8%     0.71   60.3%   86.7%      38.7%
dec16           5    5  100.0%     1411   21.8%     1.00   40.0%  100.0%      46.6%
dec17         134  131   99.7%     8684   20.7%     0.93   82.0%   95.4%      41.7%
dec18           3    3  100.0%      255    0.0%     1.00   33.3%  100.0%       0.0%
dec19           4    4  100.0%      488   29.3%     1.00   75.0%  100.0%      57.5%
dec20          13   13  100.0%     1344    8.6%     0.99   53.7%   92.3%      57.3%
dec21         170  168   99.8%     2675   25.7%     0.71   29.9%   85.7%      28.7%
dec22           4    4  100.0%       13    0.0%     1.00   50.0%  100.0%       0.0%
dec23           4    4  100.0%        9    0.0%     1.00   75.0%  100.0%       0.0%
m23_white    2212 2206   99.9%     8664   16.3%     0.64   32.7%   91.6%      36.7%
m23_black    1263 1263  100.0%     5476   33.0%     0.58   27.1%   89.0%      26.9%
m24_white     470  470  100.0%     2952   19.3%     0.73   20.9%   91.3%      79.1%
m24_black     128  128  100.0%      688   26.9%     0.64   29.8%   87.5%      36.5%
m25_white      35   35  100.0%       84   25.0%     0.42   14.8%   80.0%      37.9%
m25_black      16   16  100.0%       31   32.3%     0.33   26.4%   81.2%      51.8%
m26_white       3    3  100.0%        5   20.0%     0.50   44.4%  100.0%      18.7%
m26_black       3    3  100.0%        5   20.0%     0.50   44.4%  100.0%      18.7%
m27_white       1    1  100.0%        1    0.0%     1.00  100.0%  100.0%       0.0%
m27_black       1    1  100.0%        1    0.0%     1.00  100.0%  100.0%       0.0%
m28_white       0    0    0.0%        0    0.0%     0.00    0.0%    0.0%       0.0%
m28_black       0    0    0.0%        0    0.0%     0.00    0.0%    0.0%       0.0%
m29_white       0    0    0.0%        0    0.0%     0.00    0.0%    0.0%       0.0%
aggregate    22940 22694   99.7%   165479   28.0%     0.60   33.5%   83.3%      47.7%
```

Decisive-only aggregate (standalone `--suite decisive` run):

```
aggregate    18751 18515   99.6%   147603   28.7%     0.59   34.7%   81.8%      45.0%
```

Notes on the numbers:

- **Coverage is ~99.6–99.7%**: at 64 MB the TT evicts only ~0.3–0.4% of the
  AND children probed. TT eviction is **not** a confounder here; the measured
  sample is essentially the full tree. (The plan's eviction risk did not
  materialize at these sizes.)
- **The proxy fails on the ranking metric.** ~28% of child pairs are
  misordered, and weighted by `min(work_i, work_j)` the misordered share is
  ~45–48% — i.e. almost half the real child work sits in pairs the label ranks
  the wrong way. Both are far above the 5–10% go/no-go ceiling.
- **Pooled vs mean Kendall diverge**: pooled 0.59–0.60, mean over nodes
  0.335–0.347. Larger AND nodes (many children) agree much better and dominate
  the pooled statistic; the *typical* node — including the many 2-child AND
  nodes, where one mismatch is a coin flip of the label — agrees only weakly.
- **Top-child agreement is the one bright spot** (82–83%): the single
  largest-`subtree_size` child is usually also the max-`work` child. The proxy
  is reasonable for picking the single best child, but the corpus's AND label
  is a **full ranking**, and as a ranking it is noisy.
- **Not a partial-tree artifact**: fully solved cases without a timeout flag
  still show high flips (m23_black 33.0%, m24_black 26.9%, m25_black 32.3%),
  and shallow fully-solved mates are perfect (dec03/dec18/dec22/dec23/m27 = 0%
  flips). The noise grows with tree depth and branching, not with timeouts.
- **Structural cause**: `work` is the cumulative `child_evals` spent under the
  subtree, which includes refutation and re-expansion effort that the final
  proof tree prunes away; `subtree_size` counts only the surviving proof
  nodes. Lines that are expensive to refute but end in a small winning subtree
  are systematically undercounted by the proxy.

## Effect of TT eviction and max-updates

- **Eviction**: negligible at 64 MB (coverage ≥ 99.6% on every case with
  children; dec10 99.6%). If future runs use smaller TTs, coverage should be
  re-checked before trusting the aggregate.
- **Max-updated `work`**: `TtEntry.work` is `max`-updated per hash, so a hash
  reached along several paths records the largest `child_evals` spend, not a
  per-path value, and transposition copies share one value. This is the real
  work the search spent and rank-level agreement is robust to it, but it
  mildly inflates work for transposition-heavy hashes (e.g. dec10), which the
  top-agreement metric partially absorbs.
- **Run-to-run variance**: timeout cases rebuild slightly different trees run
  to run (dec10: 16,927 vs 16,870 AND nodes in the quick vs decisive runs);
  the aggregate flips moved ≤ 0.2 pp across runs, so conclusions are stable.

## Diff vs plan

- **Flip definition made symmetric.** The plan's literal wording
  (`subtree_size_i > subtree_size_j` disagrees with `work_i > work_j` over a
  fixed ordered pair) is direction-asymmetric for ties; this implementation
  uses `sign`-mismatch over unordered pairs, treating tie-vs-strict
  mismatches as flips in both directions (documented in the example header).
- **`kendall_mean` added.** The plan lists "Kendall τ (mean over complete
  nodes, and pooled over all complete nodes)"; the pooled value is printed as
  `kendall`, the per-node mean as `kendall_mean`.
- **`quick` = 36 cases** (decisive + move-order m≥23), matching `corpus_gen`.
- **Metrics need no board features** (only stored hashes + TT probes), but the
  tree is still replayed with a mutable `Position` to keep the report2
  replay-integrity invariant.

## Verification

- `cargo fmt --check` — clean.
- `cargo clippy --all-targets` — clean.
- `cargo doc --no-deps` — clean.
- `cargo test` — every test binary passes except `test_plan6::m22_black_loses`,
  a **pre-existing, machine-speed timeout**: the position
  `4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK b - - 0 22` is not proven
  within the test's 60 s budget on this aarch64-emulated container. The
  pristine `HEAD` build (without any plan3 change) reproduces the same failure
  and even a 180 s budget still times out, and the only plan3 `src/` change
  (`tt_work_for`, never called on the search path) cannot affect timing. This
  is unrelated to plan3; on a faster machine the 60 s regression test should
  pass as before.
- `tests/test_work_proxy_ablation.rs` (3 tests) and the unit test
  `tt_work_for_returns_stored_work` pass; the remaining binaries
  (`test_position`, `test_proof_tree`, `test_repetition`, `test_review`,
  `test_transpositions`, `verify_ppv`, and all others up to `test_plan5`)
  pass.
- Tiny end-to-end:
  `work_proxy_ablation --fen "4k3/8/8/8/8/8/8/4R1K1 w - - 0 1" --timeout 2
  --tt-size 16 --pt-size 16` — a mate-in-1 tree (2 nodes, no AND node), prints
  zeros; exit 0.
- `--suite quick --timeout 10` and `--suite decisive --timeout 10` — the tables
  above; dec10 (the transposition-heavy copied-subtree case) replays cleanly at
  ~16.9k AND nodes with 99.6% coverage.
- `--fen "r5r1/5N1k/2p2p2/pp1p3p/3Pp3/2P1P3/P7/2bQ1R1K w - - 0 30"
  --timeout 2` — 418 AND nodes, 100% coverage, flip 20.4%, work_flip 31.0%,
  demonstrating non-degenerate metrics on a single case.

## Limitations

- The comparison is rank-level on the **covered** children only; unprobed
  children (evicted) are excluded per-node, so a node with any miss is
  incomplete and excluded entirely. At 99.6% coverage this is immaterial here.
- `work` is a per-hash max, not a per-path counter; design B removes this
  approximation.
- The measurement is on trees the current ordering can build (the echo-chamber
  caveat from report2 applies). A bad-ordering world might show different
  work/proxy ratios, but for *this* pipeline the proxy's noise is measured on
  the actual training distribution.

## Verdict and next step

**Go/no-go: no-go.** The `subtree_size` proxy is not a trustworthy AND-node
ranking label: pair flip rate 28–29% and work-weighted flip share 45–48%, both
far above the 5–10% ceiling, with a typical-node Kendall τ of only ~0.34. Even
though top-child agreement is 82–83%, the corpus's AND label is a full child
ranking, and training on it would teach a noisy target.

**Next step: escalate to design B** (`plan3.md` §Escalation):

- add `work: u64` to `NodeProven` (`src/proof_event.rs`) and `ProofNode`
  (`src/proof_tree/node.rs`), recorded at prove time as the `child_evals`
  delta of the proven subtree;
- bump the `.bin` dump to v2 (`src/proof_tree/binary.rs`,
  `docs/spec/proof_tree_dump.md`);
- `corpus_gen` emits `work` per child and the AND label becomes "rank children
  by `work`"; the trainer switches targets.

With the proxy rejected, do **not** proceed to Gate 2 on `subtree_size`.
Draft the design-B plan as the immediate next step.
