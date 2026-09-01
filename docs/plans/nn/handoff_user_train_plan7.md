# Handoff (user): Gate-4b training delta in the external trainer repo

Status: the Rust side of Gate 4b (`docs/plans/nn/plan7.md`) is done and
committed; the corpus is regenerated. What remains is the **trainer-side
delta**: re-train with the §6a v2 recipe (residual logits + work weighting)
and hand `weights.v2.bin` back. This file is your checklist as the human
doing the handoff; the coding agent in the trainer repo gets its
instructions from `docs/plans/nn/handoff_agent_train_plan7.md` (copy that
file over too).

## Background in one paragraph

Gate 4 failed: the v1 network (absolute-ranking target) ordered moves
*worse* than the tuned static scorer and its inference cost made wall time
regress. Gate 4b is one bounded iteration before the PoC closes: the
trainer switches to a **residual target** (the network learns only the
*correction* to the static ordering, not an absolute ranking) and a
**work-weighted pair loss** (heavy subtrees dominate). The architecture,
feature layout, and weight-file byte format are all unchanged — only the
training recipe and the corpus differ from the v1 run. Full rationale:
`plan7.md`, `report6.md`.

## Step 1 — Copy files into the trainer repo

From the Rust repo root (paths assume the trainer repo is checked out at
`$trainer`):

```bash
trainer=/path/to/trainer/repo

mkdir -p "$trainer/docs/spec" "$trainer/docs/plans/nn"

# normative spec — RE-COPY even if a previous handoff provisioned it:
# §5 (residual composition) and §6a (v2 recipe) changed.
cp docs/spec/nn.md "$trainer/docs/spec/"

# the plan this delta implements + the agent prompt
cp docs/plans/nn/plan7.md \
   docs/plans/nn/handoff_agent_train_plan7.md "$trainer/docs/plans/nn/"

# the REGENERATED corpus — replaces whatever corpus the trainer repo
# currently has committed (the v1 weights were trained on the older
# timeout-20 generation with 9,130 clean rows)
cp data/corpus/train.ndjson "$trainer/data/corpus/"
```

Then **commit** the copied files in the trainer repo (the corpus is
committed there by design — it is the single required input and must
survive `git clean -fdx`).

Corpus facts to sanity-check after copying (`_meta` line is authoritative):
`atomic-corpus/2`, 24,109 rows of which **9,552 clean** (`partial` rows are
dropped by the trainer), 59 cases / **54 distinct `source` values** (5
cases deduped away entirely), generated with `--suite quick --timeout 420
--budget-seconds 19200 --pt-size 1024`.

Optional context (nice for the agent, not required): `report6.md` (why v1
failed), `report7.md` (current 4b state), `report2.md` (NDJSON schema).

## Step 2 — Start the container and hand off to the agent

Container setup is unchanged from the Gate-2 handoff — follow
`docs/plans/nn/trainer_handoff.md` (image, `uv sync --locked`, the
repo-root CWD rule for uv). Nothing about the 4b delta changes the
environment.

Then point the agent at the prompt file:

> Read and follow `docs/plans/nn/handoff_agent_train_plan7.md`.

## Step 3 — Hand back to the Rust repo

The agent produces (see the prompt for exact requirements):

| Trainer repo                                        | Copy to Rust repo       | Why |
| --------------------------------------------------- | ----------------------- | --- |
| `data/corpus/weights.v2.bin`                        | `data/corpus/`          | The trained v2 model; the deliverable. |
| `data/corpus/weights.v2.bin.json`                   | `data/corpus/`          | Provenance: `recipe: "residual-v2"`, λ, weighting, corpus meta, split, losses. |
| `docs/plans/nn/report_gate4b.md` (trainer-side run report) | — (stays in the trainer repo, reference for questions) | Run metrics and the exact input contract consumed. |

```bash
cp $trainer/data/corpus/weights.v2.bin* data/corpus/
```

**Do not** copy anything over `docs/nn_trainer_ref/` in the Rust repo —
the seed-0 fixture there is byte-frozen and must be unchanged
(sha256 `cb6dafd458d6ad044204f65f4faf378223527eee4ef09e707c9771d4946db2e0`;
the agent verifies this on its side). No Rust code changes are needed for
4b: the residual composition at inference is already implemented and
tested.

## Step 4 — Measure (Gate-4 harness, verbatim)

Run in the Rust repo:

```bash
cargo run --release --example benchmark -- --suite move-order --first-outcome \
  --runs 3 --json --nn-weights data/corpus/weights.v2.bin
cargo run --release --example benchmark -- --suite move-order --runs 1 \
  --timeout 60 --first-outcome --json --nn-weights data/corpus/weights.v2.bin
cargo run --release --example move_order_fractions -- --suite move-order \
  --nn-weights data/corpus/weights.v2.bin
```

Success bar (unchanged from Gate 4): ≥10–15% reduction in `child_evals`
**and** wall time on the benchmark with identical `--epsilon/--tt-size`,
`wrong == 0`, with the `--timeout 60` deconfounding runs reported
alongside. The v1 baseline numbers in `report6.md` stand unchanged.

## Step 5 — Close out

Extend `docs/plans/nn/report7.md` with the measurement verdict (or record
that the bar failed and close the PoC per `plan7.md`). Update the status
lines in `plan7.md` / `concept.md`.
