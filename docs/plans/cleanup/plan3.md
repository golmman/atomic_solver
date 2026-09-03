# Cleanup Plan 3 — Revert to the pre-NN era

## Context and goal

`docs/plans/nn/report8.md` concluded that the learned move-ordering network
PoC is a failure. The NN work (plans 1–8, `src/nn/`, the corpus/oracle
examples, the `work`-recording machinery, dump v2, and the spec) should
disappear from `main`, while the general improvements made along the way
(test tiers, deterministic eval budget, new test positions, AGENTS.md rules,
atomic-movegen 2.1.0) are kept.

Strategy: history surgery instead of a big revert commit. `main` is reset to
`3ff0941` (the last commit before the first NN commit, `3bfab72 "add nn
prompt"`), all NN-era commits remain reachable on a dedicated `nn` branch,
and the non-NN improvements are cherry-picked back onto `main`.

Tip update: the plan was originally written against `2bba249` as the tip of
`main`. Since then, three commits landed on top (`3c52a0f` this plan,
`29e1f38` nn prompt + `quick_export2` Makefile tweak, `3570a49` nn report8
update); the history below `2bba249` is unchanged, so all hashes in the pick
list below remain valid. The `nn` branch now archives through `3570a49`, and
picks 12–13 carry the survivable parts of the new commits back onto `main`.

## Decisions (agreed with the user)

1. **`859b296` is skipped.** It only adds 26 FENs to
   `docs/plans/nn/prompt.md`; all 26 were later encoded into the real
   fixtures by `9020836` (`dec24`–`dec46`, `rem23`–`rem25`). Nothing is lost.
2. **The NN-era `work` recording is dropped entirely** (commit `e44d423`,
   not in the pick list): `NodeProven.work`, work accounting in dfpn,
   `tt_work_for`, proof-tree dump **v2**. `main` returns to dump
   `VERSION = 1` and the pre-NN `NodeProven` signature. Everything stays
   recoverable from the `nn` branch.
3. **Consequence for `88e6851`:** only its AGENTS.md hunk is cherry-picked.
   Its `docs/spec/nn.md` hunk is irrelevant (file does not exist on
   post-reset `main`); its `docs/spec/proof_tree_dump.md` hunk documents
   `work` semantics that no longer exist and is dropped.
4. **`e47ba08`'s `MoveScorer::score(..., is_or_node)` trait change is
   dropped** — it was introduced for the oracle-floor harness, not needed
   without NN work (YAGNI).
5. **`2bba249` ("update cleanup prompt") is added to the pick list** so this
   very task's prompt text survives on `main`. It requires restoring
   `docs/plans/cleanup/prompt.md`, which `0cad506` deleted (see step 8).
6. **Execution mode:** per AGENTS.md, the assistant does not run git write
   commands. This plan contains the exact command sequence for the user to
   execute. Both rewritten `main` and the new `nn` branch are pushed to
   origin.
7. **Commits after `2bba249`** (landed while this plan was being prepared):
   - `3570a49` ("update nn report8") is **dropped** — pure `docs/plans/nn/`
     documentation, archived on `nn`.
   - `29e1f38` ("update nn prompt and quick_export2 makefile target") is
     **picked partially**: only its `Makefile` hunk (`quick_export2` timeout
     10 → 20) is kept; its `docs/plans/nn/prompt.md` hunk is dropped.
   - `3c52a0f` ("add cleanup plan3: revert nn") is **picked in full** so this
     very plan survives the reset on `main`.

## Pick list (in execution order)

| # | Commit | Subject | Scope |
|---|--------|---------|-------|
| 1 | `17eb7d2` | update tuned config | full |
| 2 | `2cec9fe` | add new benchmark test positions | full |
| 3 | `9020836` | add decisive test positions | full, minus the nn-prompt hunk |
| 4 | `6009f4d` | streamline nn trainer_init; bump atomic-movegen to v2.1.0 | partial: Cargo.toml/lock, README.md, tests/test_plan6.rs only |
| 5 | `0cad506` | move cleanup prompt to testability | full |
| 6 | `8ab512e` | update testability prompt | full |
| 7 | `4c5c111` | add testability plan3 | full |
| 8 | `64e4ea8` | implement testability plan3 | full, minus `nn_corpus` in Makefile `.PHONY` |
| 9 | `3fcbd48` | review and improvements of testability plan3 implementation | full |
| 10 | `88e6851` | enforce specifications to be standalone documents | partial: AGENTS.md hunk only |
| 11 | `2bba249` | update cleanup prompt | full (restores deleted file) |
| 12 | `29e1f38` | update nn prompt and quick_export2 makefile target | partial: Makefile hunk only |
| 13 | `3c52a0f` | add cleanup plan3: revert nn | full (adds this plan) |

Notes on why the order matters: `6009f4d` must land before `64e4ea8`
because `64e4ea8` touches `tests/test_plan6.rs`, whose parent version
already contains the movegen-2.1.0 FEN fix (`c`/`C` → `k`/`K`); and
`2cec9fe`/`9020836` must land before `64e4ea8`, which rewrites
`tests/fixtures/decisive_remaining.txt`. Picks 12–13 are independent of
everything above them; in particular `64e4ea8` does not touch the
`quick_export2` line that pick 12 edits, so pick 12 applies cleanly after
pick 8 (and would equally apply on the bare base).

## Execution steps

### Step 0 — Pre-flight

```sh
git status                      # must be clean
git log --oneline -1            # expect 3570a49 on main
git fetch origin
```

### Step 1 — Preserve the NN era on branch `nn`

```sh
git branch nn 3570a49
```

This branch (and its history back through `3ff0941`) is the permanent
archive of the NN work: `src/nn/`, all `docs/plans/nn/` material,
`docs/spec/nn.md`, the corpus/oracle examples, and the `work`/dump-v2
machinery — including the late nn documentation updates (`3570a49`,
`29e1f38`'s prompt hunk). Nothing needs to be re-created if the topic is
ever revisited.

### Step 2 — Reset `main`

```sh
git checkout main
git reset --hard 3ff0941
```

The old tip remains reachable via `nn` and the reflog, so this is safe.

### Step 3 — Picks 1–2 (clean)

```sh
git cherry-pick -x 17eb7d2 2cec9fe
```

Both apply cleanly on the base (`config.toml`; fixtures + doc-comment
updates + `docs/plans/move_order/prompt.md`).

### Step 4 — Pick 3: `9020836` (drop the nn-prompt hunk)

```sh
git cherry-pick -x 9020836       # expect conflict on docs/plans/nn/prompt.md
git rm -f docs/plans/nn/prompt.md
git cherry-pick --continue
```

Keep everything else: `tests/fixtures/decisive_{positions,remaining}.txt`
(dec24–dec46, rem23–rem25) and the `dec01 to dec46` doc-comment updates in
`examples/common.rs` / `tests/common/mod.rs`.

### Step 5 — Pick 4: `6009f4d` (partial, staged pick)

Keep the movegen 2.1.0 bump and its required test fix; drop all nn-doc and
AGENTS.md changes.

```sh
git cherry-pick -n 6009f4d
git restore --source=HEAD --staged --worktree -- AGENTS.md
git rm -rfq --ignore-unmatch docs/plans/nn docs/spec/nn.md
git commit -C 6009f4d -m "bump atomic-movegen to v2.1.0

(cherry picked from commit 6009f4d; nn trainer_init / docs changes dropped)"
```

Kept: `Cargo.toml` + `Cargo.lock` (movegen `2.0.0` → `2.1.0`), `README.md`
version line, `tests/test_plan6.rs` FEN fix (2.1.0 spells commoners
`k`/`K`, never `c`/`C`). Dropped: the "External NN trainer" AGENTS.md
section, `docs/plans/nn/trainer_init.md`, and the nn doc/spec edits.

### Step 6 — Picks 5–7 (clean)

```sh
git cherry-pick -x 0cad506 8ab512e 4c5c111
```

Moves `docs/plans/cleanup/prompt.md` into the testability prompt, updates
it, and adds `docs/plans/testability/plan3.md`. Note that pick 5 deletes
`docs/plans/cleanup/prompt.md`; it is restored by pick 11.

### Step 7 — Pick 8: `64e4ea8` (the testability implementation)

```sh
git cherry-pick -x 64e4ea8       # conflicts expected in Makefile and
                                 # tests/fixtures/decisive_remaining.txt
```

Conflict resolution:

- **`Makefile`** — the incoming `.PHONY` line references `nn_corpus`
  (which existed at the pick's parent but not on our base). Final content:
  base targets (`quick_export`, `quick_export2`, `macos_cleanup`) plus the
  three new targets, without `nn_corpus`:

  ```make
  .PHONY: quick_export quick_export2 macos_cleanup test test-full test-lite

  test:       ## fast gate: unit + fast integration tests (~1 min of test time)
  	CARGO_PROFILE_RELEASE_LTO=thin cargo test --release

  test-full:  ## everything, incl. 60 s regression/stress suites (~25 min)
  	cargo test --release -- --include-ignored

  test-lite:  ## debug build, quick logic check
  	cargo test
  ```

- **`tests/fixtures/decisive_remaining.txt`** — take the incoming version
  wholesale (it already contains `rem23`–`rem25` from pick 3):
  `git checkout 64e4ea8 -- tests/fixtures/decisive_remaining.txt`

- Any other conflict (`src/main.rs`, `src/search/dfpn/mod.rs`,
  `tests/common/mod.rs`, `tests/test_plan6.rs`) resolves to the incoming
  side; those files only differ on `main` by nn code we intentionally do
  not have.

This pick brings the test tiers (`make test` / `test-full` / `test-lite`),
`Search::set_child_eval_budget` + `ExitReason::BudgetExhausted`,
`#[ignore = "slow: ..."]` annotation discipline, `tests/test_smoke.rs`,
the shared test helpers, the AGENTS.md "Testing tiers" section, and
`docs/plans/testability/report3.md`.

### Step 8 — Picks 9–10

```sh
git cherry-pick -x 3fcbd48       # clean
git cherry-pick -x 88e6851       # expect conflicts under docs/spec/
git restore --source=HEAD --staged --worktree -- docs/spec/
git cherry-pick --continue
```

Kept from `88e6851`: the AGENTS.md rule that specs in `docs/spec/` must be
standalone documents. Dropped: the `docs/spec/nn.md` edits (file absent)
and the `docs/spec/proof_tree_dump.md` `work`-semantics edits (feature
dropped, decision 2/3).

### Step 9 — Picks 11–13

```sh
git cherry-pick -x 2bba249       # "deleted by us" conflict on
                                  # docs/plans/cleanup/prompt.md
git checkout 2bba249 -- docs/plans/cleanup/prompt.md
git cherry-pick --continue
```

Pick 12 (`29e1f38`, partial — Makefile hunk only):

```sh
git cherry-pick -n 29e1f38       # brings in Makefile hunk + nn prompt edit
git rm -f --ignore-unmatch docs/plans/nn/prompt.md
git commit -C 29e1f38 -m "bump quick_export2 timeout to 20 s

(cherry picked from commit 29e1f38; nn prompt.md changes dropped)"
```

The `quick_export2` hunk applies cleanly because pick 8 leaves that line
untouched. `docs/plans/nn/prompt.md` does not exist on post-reset `main`;
the `-n` pick re-creates it, so it must be removed before committing.

Pick 13 (`3c52a0f`, clean):

```sh
git cherry-pick -x 3c52a0f       # adds docs/plans/cleanup/plan3.md
```

This restores the plan being executed; it must be the last pick so the
picked text is this updated version.

### Step 10 — Final touch-up commit

One small follow-up commit on `main`:

- **AGENTS.md**: reword the spec-standalone rule's example, which currently
  reads "(e.g. the NN trainer)". Replace with a consumer that still exists,
  e.g. "(e.g. the external optimizer reading `docs/spec/
  optimizer_interface.md`)".
- Confirm `docs/plans/testability/plan3.md` keeps its historical mention of
  the `nn_corpus` make target (line ~116): accepted — plans are historical
  records and must not be rewritten.

```sh
git commit -am "cleanup: drop nn-era example wording in AGENTS.md"
```

## Validation (before pushing)

```sh
cargo build --release            # fetches atomic-movegen 2.1.0
make test                        # fast gate, target < ~60 s test time
cargo clippy --all-targets
cargo fmt --check
```

Leftover sweep — all of these must come back empty:

```sh
git grep -nEi 'nn[-_]?weights|NnWeights|corpus_gen|atomic-corpus|work_proxy|oracle_floor|move_order_fractions|set_nn_scorer|nn\.md' -- src examples tests AGENTS.md README.md Makefile Cargo.toml
git grep -n 'trainer_init' -- AGENTS.md
```

Structural spot checks:

```sh
git diff 3ff0941 main --stat                 # review: only expected deltas
git grep -n 'VERSION: u8' -- src/proof_tree/binary.rs   # expect 1
git grep -n 'pub work' -- src/proof_event.rs            # expect nothing
git grep -n 'fn score' -- src/search/ordering.rs        # 4-arg trait, no is_or_node
git grep -n 'movegen' -- README.md Cargo.toml           # expect 2.1.0
git grep -n 'quick_export2' -- Makefile                 # present, --timeout 20
test -f docs/plans/cleanup/plan3.md                     # pick 13 landed
```

Optionally run one solver smoke test via the CLI (`make quick_export`).

## Push

```sh
git push --force-with-lease origin main
git push origin nn
```

`--force-with-lease` refuses the push if origin/main moved in the
meantime. The old main history stays on the remote until the force-push
succeeds, and remains locally via `nn` + reflog regardless.

## Rollback

If anything goes wrong before the push: `git reset --hard nn` on `main`
restores the pre-surgery state. After the push, the pre-surgery main is
still the tip of the remote's reflog and of local `nn`.

## Known non-goals / accepted residue

- `docs/plans/testability/plan3.md` and `report3.md` mention nn-era
  context (`nn_corpus` target, corpus fixtures). Historical plan/report
  documents are not rewritten.
- Local, git-ignored `data/corpus/` and `data/oracle/` directories may
  remain on disk; delete manually if desired.
- The `pv_plan6` branch is untouched.

## Final task

Per the repo conventions, write `docs/plans/cleanup/report3.md` after the
surgery: actual cherry-pick outcomes, any conflicts beyond those predicted
here, validation results, deviations from this plan, and next steps.
