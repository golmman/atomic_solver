# Plan 1 Report: Node-classification profiler

## Summary

A temporary instrumentation pass was added to classify every `evaluate_child`
call and every `dfpn` frame into coarse categories, run on the two validation
cases (`m22_white` and the stress case) in first-outcome mode, then reverted.
The instrumentation did not perturb the search trajectory: the `quick` suite
produced bit-identical `child_evals` per case (aggregate 38,974,090) versus a
clean baseline.

## Raw profiler dumps

### m22_white

```
  child_total            = 14156269
  child_tt_solved        = 75572   0.53%
  child_path_rep         = 15562   0.11%
  child_terminal_fast    = 31644   0.22%
  child_preflight        = 0   0.00%
  child_searched_or_win  = 3130   0.02%
  child_searched_or_loss = 8258   0.06%
  child_searched_or_draw = 0   0.00%
  child_searched_and_loss= 10   0.00%
  child_searched_and_win = 26025   0.18%
  child_searched_and_draw= 0   0.00%
  frame_total            = 858117
  frame_solved_evals     = 14291498  10.74%
  frame_cut_evals        = 106824069  80.30%
  frame_budget_evals     = 11910977   8.95%
```

### Stress case

```
  child_total            = 249480478
  child_tt_solved        = 879110   0.35%
  child_path_rep         = 122274   0.05%
  child_terminal_fast    = 297046   0.12%
  child_preflight        = 0   0.00%
  child_searched_or_win  = 27165   0.01%
  child_searched_or_loss = 112610   0.05%
  child_searched_or_draw = 0   0.00%
  child_searched_and_loss= 80   0.00%
  child_searched_and_win = 212311   0.09%
  child_searched_and_draw= 0   0.00%
  frame_total            = 13907467
  frame_solved_evals     = 186508752   8.65%
  frame_cut_evals        = 1751854723  81.28%
  frame_budget_evals     = 216875593  10.06%
```

## Derived percentages and interpretation

### Child-eval level

| Category | m22_white | Stress |
|----------|-----------|--------|
| Fast exits (terminal + path-rep + TT-solved) | 0.86% | 0.52% |
| Searched decisive (or_win + and_loss) | 0.02% | 0.01% |
| Searched non-decisive (or_loss + and_win) | 0.24% | 0.14% |
| **Unsolved / unclassified** | **98.5%** | **99.3%** |

The overwhelming majority (>98%) of `evaluate_child` calls return **unsolved**
`ChildInfo` entries (neutral `(1,1)` bounds or non-degenerate TT bounds).  Only
a tiny fraction (<1%) hit a fast terminal, a path repetition, a TT-solved
entry, or a recursive `dfpn` that resolves the child.  This means the search
spends almost all of its child-eval budget on nodes that are **not** terminal
and **not** already in the TT as solved.

### Frame level

| Category | m22_white | Stress |
|----------|-----------|--------|
| frame_solved_evals | 10.74% | 8.65% |
| frame_cut_evals | 80.30% | 81.28% |
| frame_budget_evals | 8.95% | 10.06% |

**Threshold-cut frames dominate:** roughly 80% of all cumulative descendant
evals are spent inside `dfpn` frames that return because `pn >= th_pn` or
`dn >= th_dn` without ever storing a solved outcome.  Solved frames account for
only ~9–11% of descendant work, and budget/timeout cuts for another ~9–10%.

### Key takeaway

The single largest unmeasured surface is **unsolved child evaluations** — the
>98% of `evaluate_child` calls that do not hit any fast path and do not return
a solved result.  Reducing the cost of evaluating an *unsolved* child (e.g.
lazy / staged evaluation, #7) or reducing how many such children are created
(e.g. better move ordering / killer retuning, #10) would have far more impact
than optimizing the fast paths (which are already <1% of the total).  The
second largest surface is **threshold-cut frames** (~80% of descendant evals),
which suggests that shortening the distance to threshold satisfaction (e.g.
via TT eviction priors, #12, or frontier priors, #13) could also be
productive.

## Quick-suite drift check

`benchmark --suite quick --json --first-outcome` was run with and without the
instrumentation.  Aggregate `total_child_evals` matched bit-identically:

- Instrumented: **38,974,090**
- Clean baseline: **38,974,090**
- Mismatches: **0 / 59** cases

The instrumentation overhead (a handful of `u64` increments per child eval and
frame exit) does not perturb the search trajectory.

## Revert verification

All profiler fields, increments, and the dump call were removed from `src/`.

```bash
git diff --exit-code src/
```

Exit code 0 — no profiler code remains in the source tree.

## Files

- Raw output: `docs/plans/research/measurements/plan1/research_plan1_m22.txt`
- Raw output: `docs/plans/research/measurements/plan1/research_plan1_stress.txt`
- Quick-suite instrumented JSON: `/tmp/opencode/research_plan1_quick.json`
- Quick-suite clean JSON: `/tmp/opencode/research_plan1_quick_clean.json`
