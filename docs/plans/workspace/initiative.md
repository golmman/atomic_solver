# Initiative: `workspace` — cargo workspace split

## Status

**Active** — opened 2026-10-09 (owner proposal: monorepo / workspace split).

## Goal

Restructure the repository as a cargo workspace so that the proofdb tooling
becomes a first-class crate (`apps/proofdb`), with per-crate AGENTS.md
context, clean dependency boundaries, and no change to any external contract
(optimizer interface path, proof-store spec, data layout).

## Scope decision record

- **In scope (plan1):** workspace scaffolding; promote `examples/proofdb/` +
  `tests/proofdb.rs` to the `apps/proofdb` crate; Makefile / docs / AGENTS.md
  updates; gate re-run.
- **Out of scope (default):** absorbing `atomic-movegen` (published external
  repo with its own `movegen` initiative; revisit only if the owner declares
  it no longer independent — would be its own plan). Splitting the solver lib
  into multiple crates (no isolation gain at 13.7k lines / single author; the
  internal dependency-direction rules already enforce layering). Moving the
  CLI out of the root package (churn across Makefile/spec/docs for ~700 lines
  of shared-context code).

## Owner decisions to resolve before/during plan1

1. **campaign_worker / campaign_master**: move to `apps/proofdb` (they share
   the shard-export pipeline) or keep as root examples (`solve`-initiative
   prototypes)? Plan1 default: move to `apps/proofdb`.
2. **proofdb binaries as `bin` targets**: release binaries land in
   `target/release/` (not `target/release/examples/`); the operator manual
   invocation lines change. Plan1 default: bins (they are production tooling).
3. **atomic-movegen**: keep external (default) vs. pinned git/path dep vs.
   absorb. Default: keep external, unchanged.

## Items

- **item 1** — workspace split + proofdb crate promotion (executed by
  `plan1.md`; see the scope decision record and D1–D3 above).
- **item 2** — *low priority, deferred:* further crate carve-outs (CLI to
  `apps/cli`, `proof-store` lib from `proof_tree` + `tt_snapshot`). Rejected
  for plan1 (see plan1 pushback / decision record): no dependency-set
  isolation, no independent consumers, CLI churn across Makefile/spec/docs,
  and the internal layering rules already enforce the seams. Cheap to do
  later on top of the workspace scaffolding.

## Reopen/execute triggers for item 2

- A consumer appears that needs only `proof_tree` + `tt_snapshot` (narrow
  `proof-store` contract) — the classic carve-out trigger.
- The CLI grows beyond a thin driver (its own options surface, own docs, or
  a second binary ecosystem), making `apps/cli` context-worthy.
- Doc-path churn from moving the CLI is measured to be smaller than feared
  (e.g. a quickstart rewrite is wanted anyway).

## Success criteria

- `cargo test --release --workspace` (new `make test`) passes at the same
  coverage as today; solver behavior byte-identical (pure packaging change,
  no code semantics).
- `target/release/examples/benchmark` still exists after a root build
  (external optimizer contract, `docs/spec/optimizer_interface.md`).
- `apps/proofdb/AGENTS.md` exists; `examples/proofdb/` and the `#[path]`
  include hack are gone; rusqlite/sha2 are dependencies of `apps/proofdb`
  only.
