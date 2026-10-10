# plan14 measurements (2026-10-10) — Phase-0 tie-prevalence probe (item #23)

Evidence for [`../../plan14.md`](../../plan14.md) (item #23, Phase 0 only:
the pre-registered kill gate fired, so Phases 1–3 — the `--seed` product
knob and the calibration rollout — were never started).

| artifact | role |
| --- | --- |
| `probe_driver.py` | runs the pre-registered probe: quick suite + m22/m20 at a 20 M eval budget under `ATOMIC_TIE_PROBE=1`, plus the probe on/off hygiene arm; parses `tie_probe:` lines into `state/probe.json` |
| `state/probe.json` | per-case tie-prevalence record (calls, moves, tied_moves, top-2 ties, max top tie-group size, histogram) + hygiene results + hygiene failures (empty) |
| `env.json` | environment, probe design, kill-gate rule, gate evaluation |

Raw transcripts are not committed (regenerable via `probe_driver.py`).

## Commands

```bash
cargo build --release   # with the temporary tie_probe instrumentation
RAW=/tmp/plan14/probe python3 probe_driver.py
# then: revert the instrumentation, `git diff --exit-code` must pass
```

## Kill gate (pre-registered, D4)

Proceed to productization only if top-2-tie `sort_moves` calls ≥ 10 % on
≥ 2 of {m22_white, m20_white, dec13, dec10}.

| gate case | sort_moves calls | top-2 tie % | verdict |
| --- | --- | --- | --- |
| m20_white (censored at 20 M) | 1,008,426 | **6.19** | below |
| dec10 | 606,762 | **4.55** | below |
| m22_white | 857,953 | **1.54** | below |
| dec13 | 164,869 | **1.05** | below |

0 of 4 cases reach the threshold (gate needs ≥ 2) → **H0 fires**: a
tie-break channel keyed at the top of the ordering has no ties to break on
the eval-sensitive cases. Item #23 closes measured-out; Phases 1–3 are not
executed; the gate methodology stands as pinned in v1.0.

## Tie prevalence across the probe corpus (highlights)

- Aggregate tie mass is *large but deep*: 33–81 % of moves per call sit in
  some tie-group (dynamic bonuses unpopulated), yet the tie-group **at the
  top score** is a single move on 94–99 % of calls for the deep cases.
- The eval-sensitive / low-diversity cases have the *lowest* top-2 tie
  rates (dec13 1.05 %, dec14 1.75 %, m22 1.54 %, m20 6.19 %, dec10 4.55 %);
  small tactically-forced positions reach 10–30 % but are not noise-bearing
  (salt-invariant, 1 basin).
- Probe hygiene: dec15 (1,077,420 evals) and dec10 (4,262,128 evals)
  reproduce the plan12 salt-0 counts exactly with the probe disabled *and*
  enabled; instrumentation never perturbs the trajectory.

## Findings for the record

- The plan's opening probe (root ordering, dynamic bonuses at zero) showed
  large root tie blocks, but mid-search — where history/killer bonuses have
  accumulated — the top-2 tie rate collapses exactly where the salt channel
  needed help. Root tie mass was not decision-relevant.
- Deep tie-groups are plentiful; a channel that reorders *all* tie-groups
  (not only the top) would still have mechanical room to perturb, but that
  is a different, unregistered mechanism (and touches DF-PN selection more
  broadly) — recorded as a caveat in `report14.md`, not silently explored.
