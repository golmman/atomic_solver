#!/usr/bin/env python3
"""plan3 census instrumented build (throwaway; never shipped).

Patches a pristine tree with per-branch counters at the head of
`Search::evaluate_child` that classify every child evaluation by what a TT
pre-probe *could* decide **without playing the move**:

  solved   live current-generation entry, resolved by `resolved_from_entry`
           and the one-ply best-move guard does not fire
           -> the pre-probe returns the solved child
  unsolved live entry with `outcome == None` (not resolved)
           -> the pre-probe returns the reused unsolved bounds
  guard    live solved-shaped entry whose best-move guard fires (falls back)
  rep      child is a path repetition (fallback; the ladder decides before the
           TT)
  rule50   rule50 expired after the move (fallback; the ladder decides before
           the TT)

`skip rate = (solved + unsolved) / evals`. The keys are obtained by a
play/undo round trip inside the census block, so the counter works identically
on the pristine tree and on the plan3 tree (trajectory-neutral: counters only,
no search decision is changed).

Usage:
  python3 census.py patch /tmp/plan3_census        # patch a pristine tree
  (cd /tmp/plan3_census && cargo build --release --bins)
  python3 census.py run --binary /tmp/plan3_census/target/release/atomic_solver \
      --out state/census.json
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True

from driver import CASES, TT_MB  # noqa: E402  (same corpus)

CENSUS_MODULE = '''
/// plan3 census counters (throwaway instrumentation, never shipped).
pub mod census {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub static SOLVED: AtomicU64 = AtomicU64::new(0);
    pub static UNSOLVED: AtomicU64 = AtomicU64::new(0);
    pub static GUARD: AtomicU64 = AtomicU64::new(0);
    pub static REP: AtomicU64 = AtomicU64::new(0);
    pub static RULE50: AtomicU64 = AtomicU64::new(0);
    pub static EVALS: AtomicU64 = AtomicU64::new(0);

    #[must_use]
    pub fn snapshot() -> (u64, u64, u64, u64, u64, u64) {
        (
            SOLVED.load(Ordering::Relaxed),
            UNSOLVED.load(Ordering::Relaxed),
            GUARD.load(Ordering::Relaxed),
            REP.load(Ordering::Relaxed),
            RULE50.load(Ordering::Relaxed),
            EVALS.load(Ordering::Relaxed),
        )
    }
}
'''

CENSUS_BLOCK = '''        // ---- plan3 census instrumentation (throwaway build) ----
        {
            use census::{EVALS, GUARD, REP, RULE50, SOLVED, UNSOLVED};
            use std::sync::atomic::Ordering;
            EVALS.fetch_add(1, Ordering::Relaxed);
            pos.do_move(mv);
            let child_key = pos.hash();
            let child_rep = pos.repetition_key();
            let child_rule50 = pos.board().rule50();
            if child_rule50 >= 100 {
                RULE50.fetch_add(1, Ordering::Relaxed);
            } else if self.path_contains(child_rep) {
                REP.fetch_add(1, Ordering::Relaxed);
            } else if let Some(e) = self.tt.probe(child_key).copied() {
                let child_max_depth = max_depth.saturating_sub(1);
                let mut ok = Self::resolved_from_entry(&e, child_max_depth).is_some();
                // The shipped guard plays `best_move` from the *child*
                // position, so the census must too.
                if ok && e.best_move != Move::NONE && self.best_move_repeats_path(pos, e.best_move)
                {
                    ok = false;
                }
                if ok {
                    SOLVED.fetch_add(1, Ordering::Relaxed);
                } else if e.outcome.is_none() {
                    UNSOLVED.fetch_add(1, Ordering::Relaxed);
                } else {
                    GUARD.fetch_add(1, Ordering::Relaxed);
                }
            }
            pos.undo_move(mv);
        }
'''


def patch(tree):
    children = os.path.join(tree, "src/search/dfpn/children.rs")
    mod_rs = os.path.join(tree, "src/search/dfpn/mod.rs")
    main_rs = os.path.join(tree, "src/main.rs")

    src = open(children).read()
    # census module next to the imports
    anchor = "use crate::zobrist::INF;\n"
    assert anchor in src
    src = src.replace(anchor, anchor + CENSUS_MODULE, 1)
    src = src.replace("mod children;\n", "mod children;\n", 1)
    # counter block at the head of evaluate_child
    head = """    ) -> ChildInfo {
        self.child_evals += 1;
"""
    assert head in src, "evaluate_child head not found"
    src = src.replace(head, head + CENSUS_BLOCK, 1)
    open(children, "w").write(src)

    src = open(mod_rs).read()
    assert "mod children;\n" in src
    src = src.replace("mod children;\n", "pub mod children;\n", 1)
    open(mod_rs, "w").write(src)

    src = open(main_rs).read()
    anchor = '    eprintln!("evals: {}", search.child_evaluations());\n'
    assert anchor in src, "evals print not found"
    census_print = anchor + """    {
        let (solved, unsolved, guard, rep, rule50, evals) =
            atomic_solver::search::dfpn::children::census::snapshot();
        eprintln!(
            "census: evals={evals} solved={solved} unsolved={unsolved} guard={guard} rep={rep} rule50={rule50}"
        );
    }
"""
    src = src.replace(anchor, census_print, 1)
    open(main_rs, "w").write(src)
    print(f"patched {tree}")


RE_CENSUS = re.compile(
    r"^census: evals=(\d+) solved=(\d+) unsolved=(\d+) guard=(\d+) rep=(\d+) rule50=(\d+)$",
    re.MULTILINE,
)


def run(binary, only=None, reps=1):
    results = {}
    for name, klass, fen, extra in CASES:
        if only and name not in only:
            continue
        rows = []
        for _ in range(reps):
            cmd = [
                binary,
                "--fen",
                fen,
                *TT_MB,
                *extra,
                "--first-outcome",
                "--outcome-only",
            ]
            with tempfile.TemporaryFile() as err_fh:
                proc = subprocess.Popen(
                    cmd, stdout=subprocess.DEVNULL, stderr=err_fh
                )
                proc.wait()
                err_fh.seek(0)
                err = err_fh.read().decode()
            m = RE_CENSUS.search(err)
            if not m:
                rows.append(None)
                continue
            evals, solved, unsolved, guard, rep, rule50 = (int(g) for g in m.groups())
            rows.append(
                {
                    "evals": evals,
                    "solved": solved,
                    "unsolved": unsolved,
                    "guard": guard,
                    "rep": rep,
                    "rule50": rule50,
                    "skip_rate": round((solved + unsolved) / evals, 4) if evals else 0.0,
                }
            )
        results[name] = {"class": klass, "fen": fen, "args": TT_MB + extra, "runs": rows}
        row = rows[0]
        if row:
            print(
                f"{name:10s} evals={row['evals']:>9d} solved={row['solved']:>8d} "
                f"unsolved={row['unsolved']:>8d} guard={row['guard']:>7d} "
                f"rep={row['rep']:>6d} rule50={row['rule50']:>4d} "
                f"skip={row['skip_rate']:.1%}",
                flush=True,
            )
        else:
            print(f"{name:10s} no census line", flush=True)
    return results


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    p_patch = sub.add_parser("patch")
    p_patch.add_argument("tree")
    p_run = sub.add_parser("run")
    p_run.add_argument("--binary", required=True)
    p_run.add_argument("--out", required=True)
    p_run.add_argument("--only", default=None)
    p_run.add_argument("--reps", type=int, default=1)
    args = ap.parse_args()

    if args.cmd == "patch":
        patch(args.tree)
        return

    only = set(args.only.split(",")) if args.only else None
    results = run(args.binary, only, args.reps)
    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w") as fh:
        json.dump({"binary": args.binary, "cases": results}, fh, indent=2, sort_keys=True)
        fh.write("\n")
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()
