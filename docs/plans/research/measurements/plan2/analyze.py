#!/usr/bin/env python3
"""Outlier hard-position cluster analysis for plan2."""

import json
import re
import csv
import statistics
from pathlib import Path
from collections import Counter

MEASUREMENTS_DIR = Path(__file__).parent
RESEARCH_DIR = MEASUREMENTS_DIR.parent

# ---------------------------------------------------------------------------
# 1. Load JSON results
# ---------------------------------------------------------------------------
SUITES = ["default", "move-order", "decisive", "quick", "thorough"]

def load_json_results():
    data = {}
    for suite in SUITES:
        path = MEASUREMENTS_DIR / f"{suite}.json"
        with open(path) as f:
            data[suite] = json.load(f)["results"]
    return data

# ---------------------------------------------------------------------------
# 2. Build name -> FEN lookup from benchmark.rs and fixture files
# ---------------------------------------------------------------------------
def build_fen_lookup():
    lookup = {}
    repo_root = MEASUREMENTS_DIR.parent.parent.parent.parent.parent  # go up to repo root

    # Parse hardcoded default_suite() in examples/benchmark.rs
    bench_path = repo_root / "examples" / "benchmark.rs"
    bench_src = bench_path.read_text()

    # startpos uses Position::STARTPOS_FEN, not a literal
    lookup["startpos"] = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

    # Extract name/fen pairs from Case { name: "...", fen: "..." } blocks
    case_pattern = re.compile(
        r'Case\s*\{\s*name:\s*"([^"]+)"\.to_string\(\),\s*\n\s*fen:\s*"([^"]+)"\.to_string\(\)',
        re.MULTILINE,
    )
    for m in case_pattern.finditer(bench_src):
        lookup[m.group(1)] = m.group(2)

    # Also look for name: "..." then fen: "..." with more whitespace variation
    # Fallback: broader regex
    broad_pattern = re.compile(
        r'name:\s*"([^"]+)"\.to_string\(\),\s*.*?fen:\s*"([^"]+)"\.to_string\(\)',
        re.DOTALL,
    )
    for m in broad_pattern.finditer(bench_src):
        if m.group(1) not in lookup:
            lookup[m.group(1)] = m.group(2)

    # Parse fixture files
    fixtures = {
        "move_order_positions.txt": repo_root / "tests" / "fixtures" / "move_order_positions.txt",
        "decisive_positions.txt": repo_root / "tests" / "fixtures" / "decisive_positions.txt",
    }
    for fixture_name, fixture_path in fixtures.items():
        with open(fixture_path) as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                parts = line.split(";")
                if len(parts) >= 2:
                    name = parts[0].strip()
                    fen = parts[1].strip()
                    lookup[name] = fen

    return lookup

# ---------------------------------------------------------------------------
# 3. FEN feature extraction
# ---------------------------------------------------------------------------

def parse_fen(fen: str):
    parts = fen.split()
    placement = parts[0]
    side_to_move = parts[1] if len(parts) > 1 else "w"
    castling = parts[2] if len(parts) > 2 else "-"
    ep = parts[3] if len(parts) > 3 else "-"
    rule50 = int(parts[4]) if len(parts) > 4 else 0
    fullmove = int(parts[5]) if len(parts) > 5 else 1

    white_men = 0
    black_men = 0
    white_pawns = 0
    black_pawns = 0
    max_white_pawn_rank = 0
    min_black_pawn_rank = 9
    white_king_file = white_king_rank = None
    black_king_file = black_king_rank = None

    ranks = placement.split("/")
    for rank_idx, rank_str in enumerate(ranks):
        # rank_idx 0 = rank 8, rank_idx 7 = rank 1
        actual_rank = 8 - rank_idx
        file_idx = 0
        for ch in rank_str:
            if ch.isdigit():
                file_idx += int(ch)
            else:
                file_letter = chr(ord('a') + file_idx)
                if ch == 'K':
                    white_king_file = file_letter
                    white_king_rank = actual_rank
                    white_men += 1
                elif ch == 'k':
                    black_king_file = file_letter
                    black_king_rank = actual_rank
                    black_men += 1
                elif ch.isupper():
                    white_men += 1
                    if ch == 'P':
                        white_pawns += 1
                        max_white_pawn_rank = max(max_white_pawn_rank, actual_rank)
                else:
                    black_men += 1
                    if ch == 'p':
                        black_pawns += 1
                        min_black_pawn_rank = min(min_black_pawn_rank, actual_rank)
                file_idx += 1

    total_men = white_men + black_men
    pawnless = (white_pawns == 0 and black_pawns == 0)
    no_castling = (castling == "-")
    preflight_eligible = (total_men <= 3 and pawnless and no_castling)

    # Material config: sorted tuple of non-pawn material per side
    def extract_material(placement_str, side_upper):
        pieces = []
        for ch in placement_str:
            if ch.isalpha() and ch != 'K' and ch != 'k':
                if side_upper and ch.isupper():
                    pieces.append(ch)
                elif not side_upper and ch.islower():
                    pieces.append(ch.upper())
        return "".join(sorted(pieces))

    white_mat = extract_material(placement, True)
    black_mat = extract_material(placement, False)
    material_config = f"{white_mat} vs {black_mat}"

    has_queen_w = 'Q' in placement
    has_rook_w = 'R' in placement
    has_bishop_w = 'B' in placement
    has_knight_w = 'N' in placement
    has_queen_b = 'q' in placement
    has_rook_b = 'r' in placement
    has_bishop_b = 'b' in placement
    has_knight_b = 'n' in placement

    # King distance
    king_chebyshev_dist = None
    if white_king_file and black_king_file and white_king_rank and black_king_rank:
        file_diff = abs(ord(white_king_file) - ord(black_king_file))
        rank_diff = abs(white_king_rank - black_king_rank)
        king_chebyshev_dist = max(file_diff, rank_diff)

    return {
        "side_to_move": side_to_move,
        "total_men": total_men,
        "white_men": white_men,
        "black_men": black_men,
        "pawnless": pawnless,
        "preflight_eligible": preflight_eligible,
        "material_config": material_config,
        "white_pawns": white_pawns,
        "black_pawns": black_pawns,
        "max_white_pawn_rank": max_white_pawn_rank if white_pawns > 0 else 0,
        "min_black_pawn_rank": min_black_pawn_rank if black_pawns > 0 else 9,
        "white_king_file": white_king_file,
        "white_king_rank": white_king_rank,
        "black_king_file": black_king_file,
        "black_king_rank": black_king_rank,
        "king_chebyshev_dist": king_chebyshev_dist,
        "castling_rights": castling,
        "rule50": rule50,
        "fullmove": fullmove,
        "has_queen_w": has_queen_w,
        "has_rook_w": has_rook_w,
        "has_bishop_w": has_bishop_w,
        "has_knight_w": has_knight_w,
        "has_queen_b": has_queen_b,
        "has_rook_b": has_rook_b,
        "has_bishop_b": has_bishop_b,
        "has_knight_b": has_knight_b,
    }

# ---------------------------------------------------------------------------
# 4. Identify outliers per suite
# ---------------------------------------------------------------------------

def find_outliers(results):
    solved = [r for r in results if not r["timeout"]]
    if not solved:
        return results  # all are outliers if none solved

    child_evals = [r["child_evals"] for r in solved]
    median = statistics.median(child_evals)
    # Compute 90th percentile
    sorted_evals = sorted(child_evals)
    idx_90 = int(len(sorted_evals) * 0.9)
    p90 = sorted_evals[min(idx_90, len(sorted_evals) - 1)]
    threshold = max(2 * median, p90)

    outliers = []
    for r in results:
        if r["timeout"]:
            outliers.append(r)
        elif r["child_evals"] > threshold:
            outliers.append(r)

    return outliers

# ---------------------------------------------------------------------------
# 5. Run analysis
# ---------------------------------------------------------------------------

def main():
    data = load_json_results()
    fen_lookup = build_fen_lookup()

    # Also collect all cases with features for comparison
    all_cases_with_features = []
    all_outliers = []
    summary_lines = []

    for suite in SUITES:
        results = data[suite]
        outliers = find_outliers(results)

        solved = [r for r in results if not r["timeout"]]
        if solved:
            child_evals = [r["child_evals"] for r in solved]
            median = statistics.median(child_evals)
            sorted_evals = sorted(child_evals)
            p90 = sorted_evals[min(int(len(sorted_evals) * 0.9), len(sorted_evals) - 1)]
            threshold = max(2 * median, p90)
        else:
            median = p90 = threshold = 0

        summary_lines.append(f"=== {suite} ===")
        summary_lines.append(f"  total cases: {len(results)}")
        summary_lines.append(f"  solved: {len(solved)}")
        summary_lines.append(f"  timeouts: {len(results) - len(solved)}")
        summary_lines.append(f"  median child_evals: {median:,.0f}")
        summary_lines.append(f"  90th percentile: {p90:,.0f}")
        summary_lines.append(f"  outlier threshold: {threshold:,.0f}")
        summary_lines.append(f"  outliers: {len(outliers)}")
        summary_lines.append("")

        for r in results:
            name = r["name"]
            fen = fen_lookup.get(name)
            if not fen:
                summary_lines.append(f"  WARNING: no FEN found for {name}")
                continue
            features = parse_fen(fen)
            case_row = {
                "suite": suite,
                "name": name,
                "child_evals": r["child_evals"],
                "timeout": r["timeout"],
                "outcome": r["outcome"],
                "pv_len": r["pv_len"],
                "fen": fen,
                **features,
            }
            all_cases_with_features.append(case_row)
            if any(o["name"] == r["name"] and o["child_evals"] == r["child_evals"] for o in outliers):
                all_outliers.append(case_row)
                summary_lines.append(f"  {name}: child_evals={r['child_evals']:,} timeout={r['timeout']} outcome={r['outcome']} pv_len={r['pv_len']} mat={features['material_config']} men={features['total_men']} pawnless={features['pawnless']} preflight={features['preflight_eligible']} king_dist={features['king_chebyshev_dist']} rule50={features['rule50']}")

        summary_lines.append("")

    # Write CSV
    if all_outliers:
        fieldnames = list(all_outliers[0].keys())
        csv_path = MEASUREMENTS_DIR / "outlier_features.csv"
        with open(csv_path, "w", newline="") as f:
            writer = csv.DictWriter(f, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(all_outliers)
        summary_lines.append(f"Wrote {len(all_outliers)} rows to {csv_path}")
    else:
        summary_lines.append("No outliers found.")

    # Feature frequency tables
    summary_lines.append("")
    summary_lines.append("=== Cross-suite outlier feature frequencies ===")

    mat_counter = Counter(r["material_config"] for r in all_outliers)
    summary_lines.append("Material config:")
    for mat, cnt in mat_counter.most_common():
        summary_lines.append(f"  {mat}: {cnt}")

    total_outliers = len(all_outliers)
    if total_outliers > 0:
        pawnless_cnt = sum(1 for r in all_outliers if r["pawnless"])
        preflight_cnt = sum(1 for r in all_outliers if r["preflight_eligible"])
        high_rule50_cnt = sum(1 for r in all_outliers if r["rule50"] >= 50)
        three_or_fewer_cnt = sum(1 for r in all_outliers if r["total_men"] <= 3)
        four_or_fewer_cnt = sum(1 for r in all_outliers if r["total_men"] <= 4)
        no_castling_cnt = sum(1 for r in all_outliers if r["castling_rights"] == "-")

        summary_lines.append("")
        summary_lines.append(f"Pawnless: {pawnless_cnt}/{total_outliers} ({100*pawnless_cnt/total_outliers:.1f}%)")
        summary_lines.append(f"Pre-flight eligible (<=3 men, pawnless, no castling): {preflight_cnt}/{total_outliers} ({100*preflight_cnt/total_outliers:.1f}%)")
        summary_lines.append(f"Total men <= 3: {three_or_fewer_cnt}/{total_outliers} ({100*three_or_fewer_cnt/total_outliers:.1f}%)")
        summary_lines.append(f"Total men <= 4: {four_or_fewer_cnt}/{total_outliers} ({100*four_or_fewer_cnt/total_outliers:.1f}%)")
        summary_lines.append(f"No castling: {no_castling_cnt}/{total_outliers} ({100*no_castling_cnt/total_outliers:.1f}%)")
        summary_lines.append(f"Rule50 >= 50: {high_rule50_cnt}/{total_outliers} ({100*high_rule50_cnt/total_outliers:.1f}%)")

        # Per-suite totals
        summary_lines.append("")
        summary_lines.append("Outliers per suite:")
        suite_counter = Counter(r["suite"] for r in all_outliers)
        for s in SUITES:
            summary_lines.append(f"  {s}: {suite_counter.get(s, 0)}")

    # Compare outlier feature distribution to full-suite distribution
    summary_lines.append("")
    summary_lines.append("=== Outlier vs full-suite feature comparison ===")
    total_cases = len(all_cases_with_features)
    total_outliers = len(all_outliers)

    def compare_feature(key, label=None):
        label = label or key
        all_counter = Counter(r[key] for r in all_cases_with_features)
        out_counter = Counter(r[key] for r in all_outliers)
        summary_lines.append(f"{label}:")
        for val, out_cnt in out_counter.most_common():
            all_cnt = all_counter.get(val, 0)
            out_pct = 100 * out_cnt / total_outliers if total_outliers else 0
            all_pct = 100 * all_cnt / total_cases if total_cases else 0
            summary_lines.append(f"  {val}: {out_cnt}/{total_outliers} outliers ({out_pct:.1f}%) vs {all_cnt}/{total_cases} all ({all_pct:.1f}%)")

    compare_feature("material_config", "Material config")
    compare_feature("total_men", "Total men")
    compare_feature("pawnless", "Pawnless")
    compare_feature("preflight_eligible", "Pre-flight eligible")
    compare_feature("king_chebyshev_dist", "King Chebyshev distance")
    compare_feature("rule50", "Rule-50 clock")

    # Correlation between total_men and child_evals (all solved cases)
    solved_cases = [r for r in all_cases_with_features if not r["timeout"]]
    if solved_cases:
        import math
        n = len(solved_cases)
        xs = [r["total_men"] for r in solved_cases]
        ys = [math.log10(max(r["child_evals"], 1)) for r in solved_cases]
        mean_x = sum(xs) / n
        mean_y = sum(ys) / n
        ss_xx = sum((x - mean_x) ** 2 for x in xs)
        ss_yy = sum((y - mean_y) ** 2 for y in ys)
        ss_xy = sum((x - mean_x) * (y - mean_y) for x, y in zip(xs, ys))
        r_corr = ss_xy / math.sqrt(ss_xx * ss_yy) if ss_xx > 0 and ss_yy > 0 else 0
        summary_lines.append("")
        summary_lines.append(f"Correlation total_men vs log10(child_evals) among solved cases: r={r_corr:.3f}")

    summary_path = MEASUREMENTS_DIR / "summary.txt"
    with open(summary_path, "w") as f:
        f.write("\n".join(summary_lines) + "\n")

    print(f"Wrote summary to {summary_path}")
    print(f"Found {total_outliers} outliers total")
    print("\n".join(summary_lines))

if __name__ == "__main__":
    main()
