# Plan 6: the PNS selection mechanism — eligibility, pacing, rationing; configured and A/B'd

Initiative: `proofdb`. Executes the plan5 §2 decision as refined by the
2026-10-01 design dialogue: breadth-first PNS expansion stays the default
frontier policy; ladder escalation becomes an **explicit, configured
mechanism** (eligibility trigger + pacing + budget rationing) instead of
an emergent queue property. The plan implements the config surface, runs
the pre-registered A/B over the exposed frontier, and settles the ladder's
marginal value with data — a measured no-go being a valid outcome.
**Precondition: plan5 executed** (`report5.md` + `measurements/plan5/`
exist; the batch-2 control state is this plan's input).

Docs + `examples/`-side code only; the product solver, DB schema, and spec
are untouched; no in-session re-propagation (plan4 simplification stands).
Per repo convention the final task is `report6.md`.

## 1. Background (self-contained)

**State after plan5 (verify, never assume).** The batch-2 control ran the
unchanged plan4 policy over the exposed frontier from the plan4 ledger
snapshot. Pre-registered outcome (E1–E5): ≈ 75 censored visits, all within
the ply-2 fresh sublayer, ≈ 0 facts, no rung fired, post-batch ledger
**3,033 records** (150 censored — root + 20 ply-1 + 129 ply-2 — and 2,883
fresh; `measurements/plan5/ledger_snapshot.json` is the standing state; its
measured census — not the predictions — is this plan's input).
The two lemmas stand: **L1** no key built from (structural numbers, ledger
state) can order a fresh pool (all its ledger state is zero); **L2** a
factless policy's number-1 pool grows monotonically (≈ +21.2 exposed per
censor in batch 2, +18.2 in batch 1, vs ≤ 1 removed), so emergent
escalation — the ladder firing when "nothing cheaper remains" — is
unreachable in-session at this frontier.

**The refined mechanism (design dialogue, 2026-10-01).** Three insights,
each traceable in the dialogue record:

1. *Expansion is the parent's proof work* (AND nodes: refuting every reply
   is literally the parent's proof; OR nodes: the proof must come through
   some child) — so a censored node earns a revisit only when its whole
   exposed line is opened: **rung-eligible = censored and no virgin
   child**. While virgin children exist, harvesting them is strictly
   better use of budget than re-searching the parent.
2. *Rungs must be scheduled, not emergent* (L2): a reserve share of the
   session cap is spent on rungs, interleaved with expansion.
3. *Expansion itself is rationed per ply layer* (the report4 finding-3
   pathology: layer cost grows ~27× per ply against a fixed cap, so an
   unrationed session dies inside one layer): per-ply-layer visit caps
   make a session reach deeper plies, where plan2/plan3 measured the
   cheap facts (plies 4–27, the ply-5 sibling class).

This hybrid is new — no direct literature precedent was found for the
eligibility trigger; the escalation half is DF-PN's threshold escalation
(Nagai) in harvest-policy clothing. Hence the config surface (§2), the
per-job split trace (§2), and the pre-registered A/B (§4). The provenance
chain and theory linkage are mined into `research_pns_ladder.md` (D3).

## 2. The selection mechanism (normative, pre-registered)

**Node states (ledger-derived, as today).** Virgin (never visited; the
number-1 pool), censored (`passes ≥ 1`), decided (fact; leaves the queue),
**rung-eligible** (censored AND no virgin child). Numbers stay frozen at
session start; a node's number moves only through its own censor bumps.

**Decision function per visit** — deterministic, total, no wall clock,
ties resolved by path before any run:

```
per visit, with V = visits so far (expansion + rung),
              R = reserve child-evals spent:
1. if R >= reserve_share × session_cap                → goto 4
2. if V % interleave_k == 0 and eligible set ≠ ∅      → LADDER
3. if no expansion visit is possible (every ply
   layer's cap spent, or queue empty)
   and eligible set ≠ ∅ and reserve remains           → LADDER (fallback drain)
4. EXPAND: pop min (eff number, ply, path) among
   nodes in a ply layer with layer visits < cap;
   if none exists and nothing is ladderable           → stop ("exhausted")
```

- **Ladderable** = rung-eligible and reserve remains (the term used in
  steps 3–4). With the §4 budgets the reserve (0.25 × 300M = 75M) affords
  ≈ 9 rung visits at the 8M/16M/32M revisit ladder (revisits start at
  `2^1 × base` because pass 1 is the original censored visit) — arm-B rung
  counts are reserve-bound, not eligibility-bound.
- **Ladder target:** eligible node with fewest passes (rotation — no node
  climbs 8M → 16M → 32M while others wait), ties by (ply, path). Budget:
  `2^(passes) × base` (passes *before* this visit), never above
  `max_rung_passes` per node per session.
- **Accounting:** a rung visit spends only the reserve; an expansion
  visit counts against its ply-layer's cap; `--max-total-evals` bounds
  everything as today. Both visit kinds count into `V` for pacing.
- **Caps, not quotas:** unspent reserve and unmet rung slots at session
  end stay unspent; nothing is forced.
- **A censored rung visit** behaves exactly like a censored expansion
  visit otherwise: ledger bump (`passes += 1`, work added), re-insertion,
  and frontier exposure of its legal children (plan4 decision 11
  stands). Eligibility is re-derived from ledger + queue state after
  every visit; no new state is introduced.

**Config surface** (`--pns-config <file>`, TOML via the `toml` crate the
repo already carries for `benchmark --config`; defaults compiled in;
unknown keys rejected; the effective config echoed on the session-start
`pns:` line):

| key | default | meaning |
| --- | --- | --- |
| `reserve_share` | 0.25 | fraction of the session cap usable by rungs |
| `layer_visit_cap` | 24 | max expansion visits per ply layer per session (0 = unlimited) |
| `interleave_k` | 4 | every k-th visit considers a rung |
| `eligibility` | `"no-virgin-child"` | or `"always"` (all censored nodes eligible) |
| `rung_growth` | `"geometric"` | `2^(k-1)·base` · `"linear"` (`k·base`) · `"constant"` |
| `max_rung_passes` | 3 | per-node per-session rung cap |
| `rotation` | `"fewest-passes"` | or `"number-ply-path"` |

Base budget and session cap remain `--budget-evals` / `--max-total-evals`.

**Degenerate configs are exact legacy policies (the equivalence
contract):** `reserve_share = 0, layer_visit_cap = 0` reduces the decision
function to the plan4 selector verbatim. Arm A′ verifies this by replay.

**Census trace of the split:** every `job:` line gains `kind`
(`expand` | `rung`; legacy policies emit without it), so a batch
transcript *is* a trace of when and where the split fired — the
empirical counterpart of the normative table above. The A′≡A equivalence
comparison (§4.2, H5) is therefore taken field-wise **modulo `kind` and
wall time**; the post-run ledger byte-match is unaffected.

## 3. Deliverables

- **D1 — mechanism + config**: `pns/selector.rs` (decision function,
  eligibility, rotation, pacing) + new `pns/config.rs` (TOML, validation,
  echo) + `--pns-config` on `proofdb_harvest`; census field `kind`.
  Files ≤ 10 KB each (split `selector/pacing.rs`-style submodules if
  needed, as plan4 did); tests via `tests/proofdb.rs`: decision-table
  unit tests (eligibility edges incl. the last-virgin-child transition,
  rotation, pacing, fallback drain, degenerate equivalence, config
  validation).
- **D2 — the A/B** under `measurements/plan6/`: per-arm census JSON +
  post-run ledger snapshots + `env.json` + `README.md` (provenance,
  command table, verdicts).
- **D3 — `research_pns_ladder.md`** in the initiative dir: the provenance
  chain (report3 pivot → report4 finding 3 → plan5 lemmas → the design
  dialogue → §2's mechanism), the theory linkage (PN-search breadth
  explosion; DF-PN threshold escalation), the considered-and-rejected
  alternatives (pure expansion; pure/emergent ladder; work-aware
  ordering — plan5 L1). Mines the vendored theory under `docs/theory/`;
  vendor Nagai's DF-PN thesis if missing (slug per
  `docs/theory/README.md`); update mined entries' statuses in
  `docs/bibliography.md`.
- **D4 — `report6.md`**: arm verdicts, the ladder's marginal value, the
  default-config decision, gate verdicts, findings.

## 4. Pre-registered arms and decisions (fixed before any run)

All arms start from the **identical post-batch-2 state** (`data/proofdb.db`
+ the plan5 ledger snapshot), run on **throwaway staging copies** (fresh
TT, cap 300M, base 4M, root = startpos), and are replayed per H5.
Validated facts from any arm are promoted into the standing shard set
after the verdict (facts are arm-independent truth); a same-path outcome
contradiction between arms aborts the session (the standing soundness
tripwire). The standing `data/` ledger advances to the **winning arm's**
post-run ledger; cross-arm decided paths self-heal at the next session's
lineage gate.

1. **Arm A — status-quo control (old selector, post-batch-2 state):** the
   plan4 policy as-is. This is the reference for reach and yield from the
   same starting state (batch 2 itself was measured from the *pre*-batch-2
   state, so it anchors the S1 stage, not this comparison). Expected:
   ≈ 75 censored visits, **all within the ply-2 fresh sublayer** (214
   fresh ply-2 records > 75 visits; ply 3 not reached), ≈ 0 facts, no
   rung.
2. **Arm A′ — equivalence replay (new selector, degenerate config):**
   `reserve_share = 0, layer_visit_cap = 0`. Must reproduce arm A
   bit-for-bit: identical job sequence (every field except wall time and
   the `kind` census field — arm A's legacy selector does not emit it) and
   byte-identical post-run ledger. This pins the refactor equivalence —
   the new selector is verified as a strict superset of the old one.
3. **Arm B — mechanism defaults:** the §2 defaults table verbatim.
   Expectations: expansion reach = 24 visits at ply 2 + 24 at ply 3 + the
   sparse ply-4+ rows (3–4 per ply layer, caps never bind there) spread
   into single-digit layers up to ≈ ply 8 — **the session leaves ply 3**,
   unlike every prior session; rungs fire on the root and any node whose
   children the session fully visited (**the root's eligibility is
   verified from the standing ledger**: root and all 20 ply-1 children
   are censored, so no virgin child); expect ≈ 9 rungs (reserve-bound);
   yield ≈ 0 facts (pre-registered; plan2's 40M heavy-tier precedent says
   the quiet class censors at 10× base).
4. **Arm C — ladder deletion:** `reserve_share = 0, layer_visit_cap = 24`
   — identical expansion behavior to B minus rungs. This arm carries the
   user's position ("expansion determines everything") as a measurable
   policy.
5. **Decision rule (the ladder's marginal value), fixed now:** compare
   B vs C on (i) facts, (ii) max ply reached, (iii) evals per ply layer,
   (iv) rung yield. If B ≤ C on facts and every B rung censors with no
   depth dividend (no measurably deeper settled line), the **ladder is a
   measured no-go**: the default config becomes arm C's and the mechanism
   ships as rationed expansion only. If B's rungs produce facts or deeper
   settled lines, the ladder stays at the measured share. Either outcome
   closes the question plan5 opened.
6. **No mid-run tuning:** each arm runs its config verbatim; the config
   surface exists so *later* batches can vary one knob at a time
   (`eligibility` variants, `rung_growth`, pacing mode), never this one.
7. **Determinism:** pacing counts all visits; ordering ties resolved by
   path before any run; the census config echo + `kind` field make every
   arm's transcript self-documenting.

## 5. Pre-registered gates (fixed before any run)

- **H1 integrity:** plan4 H1 (lineage gate, exclusion census, job-path
  checks) per arm; plus the §4 promotion/tripwire rules.
- **H2 merge determinism:** merger re-run twice → byte-identical DB +
  dump; with 0 promoted facts, byte-identity with the input DB (the
  censored-session residue check).
- **H3/H4 no-regression / spot-checks:** verbatim plan4 H3/H4 over
  promoted facts; vacuous at 0.
- **H5 policy determinism:** per-arm replay (same staging input →
  identical job sequence, byte-identical post-run ledger); **plus the
  A′ ≡ A equivalence match** (all record fields except wall time and
  `kind`).
- **H6 hygiene:** `make test` green (including the new decision-table
  tests); `cargo clippy --release --all-targets` / `cargo fmt --check`
  clean; no `src/` changes; new/edited example files ≤ 10 KB;
  `git status` confirms `data/` ignored.

A gate failure is a defect in the tool or this plan's model — stop and
investigate, do not loosen the gate.

## 6. Non-goals

No in-session re-propagation (numbers stay session-frozen; the
eligibility trigger is derived from ledger + queue state, not propagated
numbers); no work-aware effective numbers (rejected for the fresh pool by
plan5 L1; re-rankable later *among rungs only* if the A/B motivates it);
no `src/` changes; no schema/spec change; no ledger-format change; no new
knobs benchmarked this batch (§4.6); no parallel harvesters (item 6); no
subtree scoping (item 7); no website handoff (item 5); no DTM-upgrade
pass (item 3).

## 7. Tasks

1. Verify the precondition (report5 + plan5 measurements exist; standing
   DB/ledger digest match the plan5 post-batch state).
2. Implement D1 (selector decision function + `pns/config.rs` +
   `--pns-config` + census `kind` + tests; split submodules to stay
   ≤ 10 KB).
3. Run the four arms (§4) on staging copies; write per-arm snapshots.
4. Check gates H1–H6; promote validated facts + the winning arm's ledger;
   merger runs (H2); assemble D2.
5. Write D3 (`research_pns_ladder.md` + bibliography updates).
6. Write D4 (`report6.md`): arm verdicts, the ladder's marginal value per
   §4.5, the default-config decision, findings.

## 8. Budget

One session. Implementation dominates (decision function + config + tests;
the selector's touch points are all in `pns/selector.rs` — ~7 KB today).
Compute: 4 arms × ≈ 70 s + H5 replays (A′ and one B/C replay) + merger
runs ≈ 10–15 min. Fits one sitting with margin.

## SESSION COMPLETE

- `docs/plans/proofdb/plan6.md` drafted (plan session, docs-only;
  plan5.md amended in place — §2 decision refined into the
  eligibility/pacing/rationing mechanism, batch-2 control untouched —
  and `initiative.md` history updated). Normative content: the decision
  table (§2), the config surface with defaults, the equivalence contract,
  and the four-arm A/B (status-quo control from S2 / degenerate
  equivalence replay / mechanism defaults / ladder deletion) with the
  fixed decision rule for the ladder's marginal value (§4.5); research
  note + bibliography mining as D3.
Follow-up options:
1. Kickoff prompt: "Execute docs/plans/proofdb/plan5.md first (batch 2
   control + report5.md), then docs/plans/proofdb/plan6.md (selector
   mechanism + --pns-config + the four-arm A/B; gates H1–H6 incl. the
   A′≡A equivalence replay; report6.md with the ladder's marginal-value
   verdict)."
2. Alternative: execute plan5 only, and review plan6's constants
   (reserve_share 0.25, layer_visit_cap 24, interleave_k 4,
   max_rung_passes 3) against batch 2's measured layer sizes before
   committing the A/B — safer if the measured post-batch-2 census
   deviates from the pre-registered ranges.
