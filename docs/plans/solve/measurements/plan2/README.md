# plan2 raw measurements (2026-09-22)

Solved-Set Frontier Push (SSFP) substrate gate + pilot for `solve`
backlog #6 (see `../../plan2.md`). **Execution status: the pre-registered
M1 gate fired NO-GO** (`m1.json`, verdict in `../../report2.md`); the
pilot loop (Arm A / Arm B / substrate) therefore **did not run**, and
`pilot.py` / the `sstore.py` merge+export paths are unexercised beyond
`sstore.py selftest` (archived as the concrete v0 mechanism record for
the rethink). The snapshot binaries were pruned after the analysis
(plan1 convention): the `snaps/*.tt.keep` markers + `state/regen_*.json`
commands reproduce them.

Harness: `ssfp.py` (Python 3
stdlib-only, black-box driver of the unmodified release binaries),
campaign tooling `sstore.py` (S store merge/export) and `pilot.py`
(Arm A / Arm B / substrate drivers). Environment: `env.json`. Layout
and conventions follow `../plan1/README.md`: raw stdout/stderr captures
under `logs/`, state JSONs under `state/`, snapshots under `snaps/`,
the S store under `store/`.

## Metric conventions

- `nodes` = the `pre_exit: ... nodes=N` count (plan1's work proxy).
- **Censored** = `pre_exit: reason=Timeout`: unproven; the run deposits
  its frontier, never a value.
- Snapshot **files are not versioned**: they are derived artifacts.
  Unlike plan1, the censored-run snapshots are **kept** (they are the
  M1 substrate data).
- The M1 gate is pre-registered in `../../plan2.md` §4 (GO ≥ 5%, NO-GO
  < 1%) and was fixed before any run; `m1.json` records the verdict.
- **Post-plan4 caveat (2026-09-23):** `examples/pt_keys` had an
  off-by-one — it printed each node's hash *before* applying the
  node's incoming move, so every non-root node reported its parent's
  position key. The primary M1 metric (TT-snapshot keys only) is
  unaffected; the secondary proof-tree key-coverage numbers in
  `m1.json` (`report2.md` §5) used shifted keys and are qualitatively
  but not exactly comparable to position-true keys. Fixed in plan4
  (see `../../report4.md` §deviations).

## Command table

| file | command |
| --- | --- |
| `env.json` | `ssfp.py env` |
| `state/regen_*.json`, `logs/regen_*.log`, `snaps/*.tt` | `ssfp.py regen` (48 censored plan1 cost runs replayed sequentially: `atomic_solver --fen <fen> --timeout 120 --first-outcome --tt-dump-path snaps/<line>_p<ply>.tt`; snapshots kept; content-equivalent, not byte-identical — censored runs are wall-clock bounded) |
| `m1.json` | `ssfp.py m1 [--min-solved 1000]` (M1 cross-system directed value share + secondary p30/p32 tree-key share; readers parse snapshot v1 binaries and the `pt_keys` replay dump) |
| `store/s.bin` | `sstore.py merge --store store/s.bin --label <label> [--snapshot <snap.tt>]... [--tree <pt.bin>]...` |
| `store/s_export.tt` | `sstore.py export --store store/s.bin --out store/s_export.tt --root-fen <root>` |
| `state/pilot_armA.json` | `pilot.py armA` (one `--timeout 7200` solve of the d4d5 p2 root) |
| `state/pilot_armB*.json` | `pilot.py armB [--budget 7200]` (queue loop: cheapest-first targets, per-target 120 s, deposit on validate) |
| `state/substrate.json` | `pilot.py substrate` (120 s re-solve of the root, fresh TT vs `--tt-load-path store/s_export.tt`) |
| `armA_reconstructed.bin`, `armB_it*.bin` | `reconstruct_pt --snapshot <snap> --out <pt>` per completed run (`validate: ok` required for verified-class deposits) |

## Campaign tooling notes (v0)

- `sstore.py` store format v0: `ATOMSSTO` magic, per-class record
  sections (verified / provisional) in the TT snapshot's 15-byte
  solved-record layout; verified wins on key conflicts;
  `sstore.py selftest` pins class separation, dedup, conflict
  resolution, binary round-trip, and export validity.
- Verified records come from validated proof trees via
  `examples/pt_keys` (replay-derived node keys + parent/move structure;
  the binary tree dump stores no hashes, so keys are recomputed). A Win
  node's best move is its min-depth child's move; Loss nodes store the
  `Move::NONE` sentinel.
- Provisional records come from TT snapshot solved sections.
- `--tt-load-path` consumes only snapshot-format solved sections, so
  `sstore.py export` re-encodes the store (both classes — both are
  usable as in-campaign anchors; only verified compose the artifact).
- Deviation from plan §3.3: campaign tooling lives in this harness
  directory (Python stdlib) rather than `examples/`/`campaign/` — the
  pilot *is* the measurement, and `examples/` stays Rust-only; a
  top-level `campaign/` dir is deferred to the prototype stage. The
  Rust-side product hooks (`--tt-load-path`, `--frontier-dump`,
  `--frontier-count`) and the `pt_keys` helper follow the plan.
