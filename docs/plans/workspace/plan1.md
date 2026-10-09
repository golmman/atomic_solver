# plan1 — Workspace split + proofdb crate promotion

Initiative: `workspace` (item 1 — the split itself; see `initiative.md` for
the scope decision record and the three owner decision points D1–D3).

## Context

`examples/proofdb/` has grown to ~6,800 lines (module tree `pns/`, `frontier`,
`descend`, `and_close`, `session`, `merge`, …) and is wired into tests via a
`#[path = "../examples/proofdb/mod.rs"]` include hack (`tests/proofdb.rs`).
It is a de-facto crate. The solver core (`src/`, ~13.7k lines) is consumed by
the proofdb tooling exclusively through the public lib API (`notation`,
`position`, `proof_tree`, `reconstruct`, `search`, `tt_snapshot`) — the crate
boundary already exists, it just isn't declared.

External contracts that must not change:

- `docs/spec/optimizer_interface.md` pins `target/release/examples/benchmark`
  as the optimizer-facing binary. A workspace shares the root `target/`, so
  this path survives **iff `benchmark` stays an example of the root package**.
- `docs/spec/global_proof_store.md` and `docs/spec/proof_tree_dump.md` pin no
  build paths (verified); the shard/DB/digest formats are unaffected.
- `data/proofdb/` tool defaults are CWD-relative; operators run from the repo
  root, which is unchanged.

## Target structure

```
Cargo.toml                # root package atomic_solver (lib + CLI + solver
                          # examples/tests) + [workspace] with members
apps/
  proofdb/                # new crate: bins + own tests + AGENTS.md
src/                     # unchanged
examples/                # solver examples only (benchmark, reconstruct_pt, …)
tests/                   # solver integration tests only
docs/                    # unchanged layout
```

Not created: `libs/` (empty dirs aren't tracked; `libs/` appears only if the
owner later decides to absorb `atomic-movegen`), `apps/cli/` (CLI stays in
the root package — decision record), top-level `examples/` member (not a
Cargo concept).

## Tasks

1. **Workspace manifest.** Root `Cargo.toml`: add `[workspace]`
   (`members = [".", "apps/proofdb"]`, `resolver = "3"` per edition 2024),
   hoist shared pinned deps into `[workspace.dependencies]`
   (`serde`, `toml`, `atomic-movegen`, and — proofdb-only — `rusqlite`,
   `sha2`), switch member manifests to `dep = { workspace = true }`. Root
   `Cargo.lock` stays the single lockfile. Confirm `cargo metadata` shows the
   expected member set and that no dep duplication crept in.

2. **Create `apps/proofdb`.** `apps/proofdb/Cargo.toml` (name `proofdb`,
   edition 2024, `depends on atomic_solver via path = "../.."`, rusqlite
   bundled, sha2, atomic-movegen). Move:
   - `examples/proofdb/*.rs` → `apps/proofdb/src/` (module tree as-is;
     fix `crate::` / `super::` paths if the `#[path]` hack assumed the
     test-crate root);
   - the four tool front-ends (`examples/proofdb_merge.rs`,
     `proofdb_harvest.rs`, `proofdb_flip.rs`, `proofdb_ledger_union.rs`) →
     `apps/proofdb/src/bin/` (D2: bin targets, so release binaries live at
     `target/release/<name>`);
   - `examples/campaign_master.rs`, `examples/campaign_worker.rs` →
     `apps/proofdb/src/bin/` (D1 default; both use the shard-export
     pipeline);
   - `tests/proofdb.rs` → `apps/proofdb/tests/proofdb.rs`, dropping the
     `#[path]` hack for a plain `use proofdb::…` (the verbatim-module-include
     trick exists only because example targets don't run unit tests; a real
     crate doesn't need it).
   - `examples/proofdb/` directory removed afterwards.
   - `apps/proofdb/AGENTS.md`: short; defers to root AGENTS.md for global
     conventions, points to `docs/proofdb_pipeline.md` (operator manual) and
     `docs/spec/global_proof_store.md` (DB contract), states the crate's
     dependency direction (may use `atomic_solver`'s public API; the solver
     never depends on proofdb).

3. **Makefile.** `test` / `test-full` / `test-lite` gain `--workspace`
   (fast gate: `CARGO_PROFILE_RELEASE_LTO=thin cargo test --release
   --workspace`). `quick_check*`, `stress`, `macos_cleanup` unchanged
   (root package still default `cargo run`). Consider a `proofdb` convenience
   target only if the manual needs it — YAGNI otherwise.

4. **Docs.**
   - `docs/proofdb_pipeline.md`: build/invocation lines — `cargo build
     --release --examples` → `cargo build --release -p proofdb --bins`,
     `BIN=target/release/examples` → `BIN=target/release`; §5 "tools live
     under `examples/`" → `apps/proofdb/`.
   - Root `AGENTS.md`: architecture section — proofdb tooling moved from
     "examples/" to the `apps/proofdb` crate; workspace layout noted;
     `make test` description mentions `--workspace`. Keep it terse; the
     per-crate detail lives in `apps/proofdb/AGENTS.md`.
   - `README.md` / quickstarts if they reference `--example proofdb_*`
     (grep for it).
   - `docs/plans/proofdb/initiative.md` + recent proofdb plan/report files:
     add a one-line note that the tooling moved crates (historical paths in
     old plans stay as written — do not rewrite history).
   - No changes to `docs/spec/*` (verified: no path pins beyond the
     benchmark one, which is preserved).

5. **Cleanup.** Remove the empty `libs/Fairy-Stockfish` directory;
   `grep -rn "examples/proofdb\|--example proofdb\|--example campaign"` over
   the repo (docs, Makefile, tests, scripts) to catch stragglers;
   `cargo doc` builds clean.

6. **Gate.**
   - `make test` (now workspace-wide) — must pass with the same test count
     as before the split (compare totals pre/post; the proofdb tests just
     moved, none may vanish).
   - Solver byte-identity smoke: `make quick_check3` plus one
     `benchmark --json --suite quick` run; confirm
     `target/release/examples/benchmark` exists and emits valid JSON
     (optimizer contract).
   - proofdb smoke: build `apps/proofdb`, run the fixture-based
     `proofdb_merge` + `proofdb_harvest` flow from the manual §3.1 against a
     scratch `data/proofdb/` (or rely on the moved regression test if it
     covers the pipeline end-to-end — it does, via `tests/proofdb.rs`).
   - `cargo clippy --workspace --all-targets`, `cargo fmt --check`,
     `cargo doc --workspace`.

7. **Report.** Write `docs/plans/workspace/report1.md`: what moved where,
   gate results (test counts pre/post), any path fixes found in docs,
   D1–D3 resolutions as taken, problems/next steps. Update the
   `docs/plans/README.md` `workspace` row (opened → state after plan1).

## Risks / notes

- **Module-path fallout in the proofdb tree**: the `#[path]` hack compiled
  the modules as if rooted in the test crate; `apps/proofdb/src/` rooting is
  the natural fix, but `mod.rs`-relative `#[path]`s inside the tree (e.g.
  `pns/`) must be re-checked.
- **`cargo test` inside a workspace runs all members' tests** — hence the
  `--workspace` Makefile change is mandatory, not cosmetic, and the fast-gate
  time budget (<60 s) must be re-confirmed.
- **No code semantics change** anywhere: this is packaging only. If any
  `#[cfg]`/feature or behavior diff is needed to make it compile, stop and
  record it in the report — that would violate the split's contract.
- Data directories (`data/proofdb/`) and `.gitignore` patterns are
  path-stable; nothing to migrate.
