#!/usr/bin/env python3
"""Derive the plan1 census from the driver's raw result files.

Inputs (all written by `../bench/run_bench.py`):

- `results_hand_census.json`  — provari `--guide hand`, 1 repeat, all 21 cells
- `results_hand_bench.json`   — provari `--guide hand`, 3 repeats, bench cells
- `results_model_census.json` — provari `--guide model`, 1 repeat, all cells
- `results_model_bench.json`  — provari `--guide model`, 3 repeats, bench cells
- `model_column.json`         — `{"status": "INCONCLUSIVE", ...}` fallback

Outputs: `census.json` (machine) and `census.md` (report table).

Rules:
- medians and coverage are recomputed from the **raw rows**, never taken from
  the driver's own ledger;
- a bench cell's 3-repeat rows replace its 1-repeat census rows (and are
  sourced that way in the output);
- the candidate (`atomic_solver`) rows are read from the hand files: our
  runs do not depend on provari's guide, and the hand file carries the
  designated 3-repeat bench rows;
- a censored cell is reported as `censored` (never dropped, never a win);
  where a bound is meaningful it is printed as `>= budget / ref_wall`;
- model columns are context only: they never feed the bench-set rule or the
  primary coverage claim.

No raw transcripts, no `__pycache__` (`sys.dont_write_bytecode`).
"""

from __future__ import annotations

import json
import statistics
import sys
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
BUDGET_S = 10.0

CORPUS_ORDER_FALLBACK = ["core"]
EXISTING_BENCH = {
    "core": "core",
    "1eb80b92f92c": "easy-win",
    "db6a741c723f": "deep-win",
}
STACKS = ("default", "opt")


def load(path: Path) -> dict | None:
    if not path.exists():
        return None
    return json.loads(path.read_text())


def collect(files: list[Path]) -> dict[str, dict[str, list[dict]]]:
    """case -> side -> rows. Later files win a case wholesale."""
    table: dict[str, dict[str, list[dict]]] = {}
    for path in files:
        data = load(path)
        if data is None:
            continue
        cases: dict[str, dict[str, list[dict]]] = {}
        for row in data["runs"]:
            cases.setdefault(row["case"], {}).setdefault(row["side"], []).append(row)
        table.update(cases)
    return table


def summarize(group: list[dict] | None) -> dict | None:
    if not group:
        return None
    walls = sorted(r["wall_s"] for r in group)
    return {
        "runs": len(group),
        "wall_s": round(statistics.median(walls), 3),
        "wall_min_s": walls[0],
        "wall_max_s": walls[-1],
        "win": all(r["win"] for r in group),
        "nodes": next((r["nodes"] for r in group if r.get("nodes") is not None), None),
        "mate": next((r["mate"] for r in group if r.get("mate") is not None), None),
        "evals": next((r["evals"] for r in group if r.get("evals") is not None), None),
        "pv_length": next(
            (r.get("pv_length") for r in group if r.get("pv_length")), None),
        "exit": next((r.get("exit") for r in group if r.get("exit")), None),
    }


def geomean(values: list[float]) -> float | None:
    if not values:
        return None
    if len(values) == 1:
        return round(values[0], 3)
    return round(statistics.geometric_mean(values), 3)


def shared_block(case_order: list[str], ref: dict, cand: dict) -> dict:
    cells = []
    for case in case_order:
        r, c = ref.get(case), cand.get(case)
        if not r or not c:
            continue
        if r["win"] and c["win"]:
            cells.append({
                "case": case,
                "reference_wall_s": r["wall_s"],
                "candidate_wall_s": c["wall_s"],
                "factor": round(c["wall_s"] / r["wall_s"], 3),
            })
    factors = [cell["factor"] for cell in cells]
    return {
        "shared_cells": [cell["case"] for cell in cells],
        "shared_count": len(cells),
        "geomean_factor": geomean(factors),
        "cells": cells,
    }


def main() -> int:
    hand_table = collect([HERE / "results_hand_census.json",
                          HERE / "results_hand_bench.json"])
    model_table = collect([HERE / "results_model_census.json",
                           HERE / "results_model_bench.json"])
    model_note = load(HERE / "model_column.json")
    if model_note and model_note.get("status") == "INCONCLUSIVE":
        model_table = {}

    hand_data = load(HERE / "results_hand_census.json") or {"runs": []}
    case_order = list(dict.fromkeys(row["case"] for row in hand_data["runs"]))
    if not case_order:
        raise SystemExit("no hand census rows")
    bench_repeats = {row["case"] for row in
                     (load(HERE / "results_hand_bench.json") or {"runs": []})["runs"]}

    hand_ref = {c: summarize(hand_table.get(c, {}).get("provari-hand"))
                for c in case_order}
    model_ref = {c: summarize(model_table.get(c, {}).get("provari-model"))
                 for c in case_order}
    cand = {
        name: {c: summarize(hand_table.get(c, {}).get(f"atomic_solver:{name}"))
               for c in case_order}
        for name in STACKS
    }

    cells = []
    for case in case_order:
        cell = {
            "case": case,
            "source": "bench-3-repeat" if case in bench_repeats else "census-1-repeat",
            "short_name": EXISTING_BENCH.get(case),
            "sample20": case != "core",
            "provari_hand": hand_ref[case],
            "provari_model": model_ref[case],
        }
        for name in STACKS:
            entry = dict(cand[name][case] or {"win": False, "wall_s": None})
            if hand_ref[case]["win"] and not entry["win"]:
                entry["factor_lower_bound"] = round(
                    BUDGET_S / hand_ref[case]["wall_s"], 2)
            if hand_ref[case]["win"] and entry["win"]:
                entry["factor"] = round(
                    entry["wall_s"] / hand_ref[case]["wall_s"], 3)
            cell[name] = entry
        cells.append(cell)

    total = len(case_order)
    sample20 = [c for c in case_order if c != "core"]

    def coverage(table: dict) -> dict:
        wins = [c for c in case_order if table.get(c) and table[c]["win"]]
        return {"all": len(wins), "all_total": total,
                "sample20": len([c for c in wins if c != "core"]),
                "sample20_total": len(sample20), "wins": wins}

    model_status = ("INCONCLUSIVE" if model_note and
                    model_note.get("status") == "INCONCLUSIVE" else
                    ("measured" if any(v for v in model_ref.values()) else
                     "not-measured"))
    coverage_block = {
        "provari_hand": coverage(hand_ref),
        "default": coverage(cand["default"]),
        "opt": coverage(cand["opt"]),
        "provari_model": (coverage(model_ref) if model_status == "measured"
                          else {"status": model_status,
                                "reason": (model_note or {}).get("reason")}),
    }

    hand_wins = [c for c in case_order if hand_ref[c]["win"]]
    bench_cells = [{"case": c, "short_name": EXISTING_BENCH.get(c)}
                   for c in hand_wins]
    new_hand_wins = [c for c in hand_wins if c not in EXISTING_BENCH]
    removed = [c for c in EXISTING_BENCH if c not in hand_wins]

    shared = {
        "hand_x_default": shared_block(case_order, hand_ref, cand["default"]),
        "hand_x_opt": shared_block(case_order, hand_ref, cand["opt"]),
    }
    if model_status == "measured":
        shared["model_x_opt"] = shared_block(case_order, model_ref, cand["opt"])
        shared["model_x_default"] = shared_block(
            case_order, model_ref, cand["default"])

    provenance = {
        "plan": "winprover/plan1",
        "budget_s": BUDGET_S,
        "hand": (load(HERE / "results_hand_census.json") or {}).get("provenance"),
        "model": (load(HERE / "results_model_census.json") or {}).get("provenance"),
        "model_column_note": model_note,
        "bench_cells": sorted(bench_repeats),
    }

    census = {
        "provenance": provenance,
        "coverage": coverage_block,
        "cells": cells,
        "shared": shared,
        "bench_set": {
            "selection_rule": "provari --guide hand wins inside the canonical budget",
            "cells": bench_cells,
            "new_hand_wins": new_hand_wins,
            "removed_but_still_expected": removed,
        },
    }
    (HERE / "census.json").write_text(json.dumps(census, indent=1) + "\n")

    lines = [
        "# Plan1 census — provari hand vs. model, our stacks",
        "",
        f"Cells: {total} (`core` + {len(sample20)} `sample20`), budget "
        f"{BUDGET_S:.0f} s / 192 MB label both sides. Generated by "
        "`derive.py` from the raw driver rows; do not edit by hand.",
        "",
        "## Coverage (wins / cells)",
        "",
        "| Column | `core` + `sample20` | `sample20` only |",
        "| --- | ---: | ---: |",
    ]
    for name, key in (("provari `--guide hand`", "provari_hand"),
                      ("provari `--guide model`", "provari_model"),
                      ("atomic_solver default", "default"),
                      ("atomic_solver opt", "opt")):
        cov = coverage_block[key]
        if "status" in cov:
            lines.append(f"| {name} | {cov['status']} | {cov['status']} |")
        else:
            lines.append(f"| {name} | {cov['all']}/{cov['all_total']} | "
                         f"{cov['sample20']}/{cov['sample20_total']} |")
    lines += ["", "## Shared-window factors (our wall / reference wall)", ""]
    for key, block in shared.items():
        calls = ", ".join(f"`{c['case']}` {c['factor']}" for c in block["cells"])
        lines.append(f"- **{key}**: geomean **{block['geomean_factor']}** over "
                     f"{block['shared_count']} cell(s){'; ' + calls if calls else ''}")
    lines += [
        "",
        "## Per-cell table",
        "",
        "`ref` = provari wall/nodes/mate (mate in attacker moves); ours = "
        "wall/child_evals/PV plies. `cens` = censored at the budget.",
        "",
        "| Cell | provari hand | provari model | default | opt |",
        "| --- | --- | --- | --- | --- |",
    ]

    def fmt(ref: dict | None, extra: str = "") -> str:
        if ref is None:
            return "not measured"
        if not ref["win"]:
            return "cens"
        bits = f"{ref['wall_s']:.3f} s"
        if ref.get("nodes"):
            bits += f" / {ref['nodes']:,} n"
        if ref.get("mate"):
            bits += f" / mate {ref['mate']}"
        return bits + extra

    def fmt_c(entry: dict | None) -> str:
        if entry is None:
            return "not measured"
        if not entry["win"]:
            bound = entry.get("factor_lower_bound")
            return "cens" + (f" (≥ {bound})" if bound else "")
        bits = f"{entry['wall_s']:.3f} s"
        if entry.get("evals"):
            bits += f" / {entry['evals'] / 1e6:.1f} M"
        if entry.get("pv_length"):
            bits += f" / {entry['pv_length']} ply"
        return bits

    for cell in cells:
        name = cell["short_name"] or cell["case"]
        lines.append(
            f"| `{name}` | {fmt(cell['provari_hand'])} | "
            f"{fmt(cell['provari_model'])} | {fmt_c(cell['default'])} | "
            f"{fmt_c(cell['opt'])} |")
    lines += [
        "",
        "## Bench set (fixed selection rule)",
        "",
        f"- provari hand wins: {', '.join('`' + c + '`' for c in hand_wins)}",
        f"- new hand wins vs. the shipped set: "
        f"{', '.join('`' + c + '`' for c in new_hand_wins) or 'none'}",
        f"- expected-but-censored: "
        f"{', '.join('`' + c + '`' for c in removed) or 'none'}",
        "",
    ]
    (HERE / "census.md").write_text("\n".join(lines))
    print(f"wrote {HERE / 'census.json'} and census.md")
    print(f"hand coverage {coverage_block['provari_hand']['all']}/{total}; "
          f"default {coverage_block['default']['all']}/{total}; "
          f"opt {coverage_block['opt']['all']}/{total}")
    print("shared: " + ", ".join(
        f"{k}={v['geomean_factor']} ({v['shared_count']} cells)"
        for k, v in shared.items()))
    print(f"new hand wins: {new_hand_wins or 'none'}; "
          f"removed: {removed or 'none'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
