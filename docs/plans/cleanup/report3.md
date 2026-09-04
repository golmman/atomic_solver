# Cleanup Plan 3 — Implementation Report

Executed `docs/plans/cleanup/plan3.md`: history surgery reverting `main` to the
pre-NN era (`3ff0941`) while preserving the NN work on branch `nn` and
cherry-picking the non-NN improvements back onto `main`.

## Outcome

- `nn` branch created at `3570a49` (the old `main` tip) — full archive of the
  NN era, including `src/nn/`, `docs/plans/nn/`, `docs/spec/nn.md`, the
  corpus/oracle examples, and the `work`/dump-v2 machinery.
- `main` reset to `3ff0941`, then 13 picks + 3 follow-up commits:
  1. `17eb7d2` update tuned config — clean
  2. `2cec9fe` add new benchmark test positions — **1 unpredicted conflict**
     in `docs/plans/move_order/prompt.md` (incoming decisive-FEN block vs.
     base without it); resolved to the incoming side (pick is "full")
  3. `9020836` add decisive test positions — predicted modify/delete conflict
     on `docs/plans/nn/prompt.md`; `git rm`, continued (predicted)
  4. `6009f4d` partial staged pick (movegen 2.1.0 bump: `Cargo.toml`,
     `Cargo.lock`, `README.md`, `tests/test_plan6.rs`; AGENTS.md and nn docs
     dropped). Five modify/delete conflicts on nn docs, all resolved by
     `git restore AGENTS.md` + `git rm -rfq docs/plans/nn docs/spec/nn.md`
     (within predicted scope; more files than the plan listed explicitly)
  5. `0cad506` move cleanup prompt to testability — **1 unpredicted trivial
     conflict** in `docs/plans/cleanup/prompt.md` (a trailing `---` line);
     resolved by taking the deletion (the pick moves the file)
  6. `8ab512e` update testability prompt — clean
  7. `4c5c111` add testability plan3 — clean
  8. `64e4ea8` implement testability plan3 — applied **cleanly** (the plan
     predicted conflicts in `Makefile` and `tests/fixtures/decisive_remaining.txt`;
     the auto-merge produced the correct fixtures). However the auto-merge
     **silently kept `nn_corpus` in the `.PHONY` line**, so the Makefile was
     fixed to `.PHONY: quick_export quick_export2 macos_cleanup test
     test-full test-lite` and amended into the pick, exactly as the plan's
     conflict-resolution block intended
  9. `3fcbd48` review and improvements of testability plan3 — clean
  10. `88e6851` partial: AGENTS.md hunk only. Predicted `docs/spec/` conflicts
      occurred; `docs/spec/nn.md` removed, `docs/spec/proof_tree_dump.md`
      restored from HEAD, commit contains only the AGENTS.md hunk
  11. `2bba249` update cleanup prompt — predicted modify/delete conflict;
      file restored from `2bba249` and verified byte-identical
      (`git diff 2bba249:… HEAD:…` empty). Note: the pick's stat shows 30
      insertions vs. 26 in the original because `2bba249`'s parent still had
      a 4-line stub of the file; content is identical
  12. `29e1f38` partial: Makefile `quick_export2` timeout 10 → 20 only;
      re-created `docs/plans/nn/prompt.md` removed before committing
      (predicted)
  13. `3c52a0f` add cleanup plan3 — clean; the plan file includes the
      "Tip update" hunk (see deviations)
- Follow-up commits:
  - `cleanup: drop nn-era example wording in AGENTS.md` (plan step 10:
    spec-standalone example now references the external optimizer reading
    `docs/spec/optimizer_interface.md`)
  - `cleanup: ignore local data/corpus and data/oracle leftovers` (see
    deviations)

## Deviations from the plan

1. **`git fetch origin` failed** — this environment has no SSH credentials
   for `git@github.com`. Pre-flight was run against the local state; the
   pushes (`git push --force-with-lease origin main`, `git push origin nn`)
   remain for the user, as planned.
2. **Git identity** was not configured in the environment; set repo-locally
   to the sole committer identity found in history
   (`Dirk Kretschmann <kretschmanndi@gmail.com>`).
3. **`git commit -C <sha> -m "…"` is rejected** by this git version
   ("options '-C' and '-m' cannot be used together"). For picks 4 and 12 the
   staged pick was committed with `-C` and the message then set via
   `git commit --amend -m`, producing the same author/date/message result.
4. **Plan tip update preserved**: the working tree contained the uncommitted
   "Tip update" hunk for `docs/plans/cleanup/plan3.md` (picks 12–13
   additions). It was saved before the reset, re-applied after pick 13, and
   amended into the pick so the plan on `main` is the updated version, as the
   plan intended.
5. **`data/corpus/` and `data/oracle/` were not git-ignored** on the
   rewritten `main` (the plan's "accepted residue" section assumed they
   were; the `.gitignore` entries came from the dropped nn-era history).
   A one-line `.gitignore` addition was committed so `git status` stays
   clean; the directories themselves remain on disk.
6. Unpredicted conflicts in picks 2 and 5 (see above); both resolved
   conservatively within the picks' declared scope.

## Validation results

- `cargo build --release` — OK (fetched atomic-movegen 2.1.0)
- `make test` — all green: 153 unit tests + all fast integration tiers,
  0 failures, slow tests ignored as designed (~45 s wall)
- `cargo clippy --all-targets` — clean
- `cargo fmt --check` — clean
- Leftover sweep (`nn[-_]?weights|NnWeights|corpus_gen|atomic-corpus|
  work_proxy|oracle_floor|move_order_fractions|set_nn_scorer|nn\.md` over
  `src examples tests AGENTS.md README.md Makefile Cargo.toml`, plus
  `trainer_init` in AGENTS.md) — both empty
- Structural spot checks:
  - `git diff 3ff0941 main --stat` — only expected deltas (29 files, all in
    the pick scope; no `src/nn/`, no nn docs/spec/examples)
  - `src/proof_tree/binary.rs` `VERSION: u8 = 1`
  - no `pub work` in `src/proof_event.rs`
  - `MoveScorer::score` is the 4-arg trait (no `is_or_node`)
  - `atomic-movegen = "2.1.0"` in `Cargo.toml` and README
  - `quick_export2` present with `--timeout 20`
  - `docs/plans/cleanup/plan3.md` present
- `pv_plan6` branch untouched.

## Problems encountered

- None beyond the deviations above. The two unpredicted conflicts were
  trivial and the one silent auto-merge (`nn_corpus` in `.PHONY`) was caught
  by inspecting the pick diff.

## Unresolved parts

- None known. `docs/plans/testability/plan3.md`/`report3.md` keep their
  historical nn-era mentions (`nn_corpus` target, corpus fixtures) — accepted
  residue; historical plans/reports are not rewritten.

## Missing tests

- The slow tier (`make test-full`, ~25 min) was not run in this session; it
  should be run before the force-push per the AGENTS.md tier policy, since
  this change touches search/testability code (picks 8–9).
- The optional `make quick_export` CLI smoke test was not run (covered by
  the fast integration tier instead).

## Next steps

1. Run `make test-full` (recommended before pushing).
2. Push: `git push --force-with-lease origin main` and `git push origin nn`.
3. Delete the local `data/corpus/` / `data/oracle/` directories manually if
   no longer wanted (now git-ignored).
