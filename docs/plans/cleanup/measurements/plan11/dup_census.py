#!/usr/bin/env python3
"""Normalized duplicate-block census for two files (cleanup plan11 Phase C).

Usage: dup_census.py <rev> <path_a> <path_b>

`rev` is a git revision (`HEAD` reads the committed copy, `WORKTREE` reads the
file as it stands) so before/after runs differ only by the working tree.
Lines are whitespace-trimmed and blank lines dropped; a matched block counts
only with >= 2 consecutive lines (single `}`-style coincidences are noise).
"""

import difflib
import subprocess
import sys


def read(rev: str, path: str) -> list[str]:
    if rev == "WORKTREE":
        text = open(path, encoding="utf-8").read()
    else:
        text = subprocess.run(
            ["git", "show", f"{rev}:{path}"], check=True, capture_output=True, text=True
        ).stdout
    return [line.strip() for line in text.splitlines() if line.strip()]


def main() -> int:
    rev, path_a, path_b = sys.argv[1:4]
    a, b = read(rev, path_a), read(rev, path_b)
    matcher = difflib.SequenceMatcher(None, a, b, autojunk=False)
    blocks = [blk for blk in matcher.get_matching_blocks() if blk.size >= 2]
    total = sum(blk.size for blk in blocks)
    print(f"{rev}: {path_a} ({len(a)} lines) vs {path_b} ({len(b)} lines)")
    print(f"matched blocks (>=2 lines): {len(blocks)}, duplicated lines: {total}")
    for blk in blocks:
        print(f"  a[{blk.a}:{blk.a + blk.size}] == b[{blk.b}:{blk.b + blk.size}] ({blk.size} lines)")
        for line in a[blk.a : blk.a + blk.size]:
            print(f"    | {line}")
    return 0


if __name__ == "__main__":
    sys.setrecursionlimit(10000)
    raise SystemExit(main())
