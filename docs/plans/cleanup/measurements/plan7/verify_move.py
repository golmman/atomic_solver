#!/usr/bin/env python3
"""Mechanical move-only checker for the dfpn/mod.rs split (cleanup plan7).

Parses `fn` bodies by brace matching (string/comment aware), normalizes each
body line by collapsing whitespace and dropping empty lines, and hashes the
result. The signature line is excluded from the hash but the full function
line count is recorded.

  verify_move.py --snapshot HEAD
      Write state/functions_before.json from `git show HEAD:src/search/dfpn/mod.rs`.
  verify_move.py --check
      Verify every before-body appears exactly once across the split files
      (mod.rs, config.rs, run.rs), the fn-name multiset is unchanged, and no
      extra fn was introduced.
"""
import sys
sys.dont_write_bytecode = True

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
DFPN = ROOT / "src/search/dfpn"
SPLIT_FILES = ["mod.rs", "config.rs", "run.rs"]
STATE = Path(__file__).resolve().parent / "state"

FN_RE = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")


def strip_strings_and_comments(line, in_block_comment):
    """Return (code_without_braces_in_literals, in_block_comment)."""
    out = []
    i = 0
    n = len(line)
    in_str = False
    in_char = False
    while i < n:
        c = line[i]
        if in_block_comment:
            if line.startswith("*/", i):
                in_block_comment = False
                i += 2
            else:
                i += 1
            continue
        if in_str:
            if c == "\\":
                i += 2
                continue
            if c == '"':
                in_str = False
            i += 1
            continue
        if in_char:
            if c == "\\":
                i += 2
                continue
            if c == "'":
                in_char = False
            i += 1
            continue
        if line.startswith("//", i):
            break
        if line.startswith("/*", i):
            in_block_comment = True
            i += 2
            continue
        if c == '"':
            in_str = True
            i += 1
            continue
        # A char literal: 'x' or '\n' or '\''. Avoid lifetimes ('a).
        if c == "'":
            if i + 2 < n and line[i + 2] == "'" and line[i + 1] != "\\":
                in_char = True
                i += 1
                continue
            if i + 3 < n and line[i + 1] == "\\" and line[i + 3] == "'":
                in_char = True
                i += 1
                continue
            i += 1
            continue
        out.append(c)
        i += 1
    return "".join(out), in_block_comment


def parse_functions(text):
    """Return list of dicts: name, body_hash, lines (signature..closing brace)."""
    lines = text.splitlines()
    funcs = []
    i = 0
    in_block = False
    # Pre-strip comments/strings for structural scanning, keeping line indices.
    code_lines = []
    for raw in lines:
        code, in_block = strip_strings_and_comments(raw, in_block)
        code_lines.append(code)

    while i < len(lines):
        code = code_lines[i]
        m = FN_RE.search(code)
        if not m:
            i += 1
            continue
        name = m.group(1)
        # Find the opening brace that starts the body: search from the match end
        # on this line, then following lines, until a `{` appears.
        start_line = i
        open_pos = None
        depth = 0
        j = i
        scan_from = m.end()
        while j < len(code_lines):
            segment = code_lines[j]
            k = scan_from if j == i else 0
            while k < len(segment):
                ch = segment[k]
                if ch == "{":
                    if open_pos is None:
                        open_pos = (j, k)
                    depth += 1
                elif ch == "}":
                    depth -= 1
                k += 1
            if open_pos is not None and depth == 0:
                break
            j += 1
        if open_pos is None:
            i += 1
            continue
        end_line = j
        # Body text: everything after the opening brace through the closing
        # brace (exclusive), taken from the original lines.
        body_lines = []
        for ln in range(start_line, end_line + 1):
            # The signature (which may span several lines) is excluded; the
            # body starts after the opening brace and ends before the matching
            # closing brace.
            if ln < open_pos[0]:
                continue
            raw = lines[ln]
            if ln == open_pos[0]:
                raw = raw[open_pos[1] + 1:]
            if ln == end_line:
                stripped = code_lines[ln]
                close_idx = stripped.rfind("}")
                if close_idx >= 0:
                    raw = raw[:close_idx]
            body_lines.append(raw)
        # Drop the first (signature remains before the opening brace) junk:
        # if the opening line also contains signature text before `{`, our
        # slice already starts after `{`. Any remaining signature text on that
        # line only occurs for multi-line signatures, where the opening brace
        # is on its own line (empty body prefix).
        normalized = []
        for ln in body_lines:
            ln = " ".join(ln.split())
            if ln:
                normalized.append(ln)
        digest = hashlib.sha256("\n".join(normalized).encode()).hexdigest()
        funcs.append({
            "name": name,
            "body_hash": digest,
            "lines": end_line - start_line + 1,
        })
        i = end_line + 1
    return funcs


def load_before():
    return json.loads((STATE / "functions_before.json").read_text())


def do_snapshot(rev):
    src = subprocess.check_output(
        ["git", "show", f"{rev}:src/search/dfpn/mod.rs"], cwd=ROOT
    ).decode()
    funcs = parse_functions(src)
    STATE.mkdir(parents=True, exist_ok=True)
    payload = {
        "source": f"{rev}:src/search/dfpn/mod.rs",
        "functions": {f["name"]: {"body_hash": f["body_hash"], "lines": f["lines"]} for f in funcs},
    }
    (STATE / "functions_before.json").write_text(json.dumps(payload, indent=2) + "\n")
    print(f"snapshot: {len(funcs)} functions -> state/functions_before.json")


def do_check():
    before = load_before()["functions"]
    after = {}
    for fname in SPLIT_FILES:
        path = DFPN / fname
        if not path.exists():
            print(f"MISSING FILE: {path}")
            return 1
        for f in parse_functions(path.read_text()):
            after.setdefault(f["name"], []).append((fname, f))

    ok = True
    for name, info in before.items():
        hits = after.get(name, [])
        if len(hits) != 1:
            print(f"FAIL {name}: found {len(hits)} occurrences (want exactly 1)")
            ok = False
            continue
        fname, f = hits[0]
        if f["body_hash"] != info["body_hash"]:
            print(f"FAIL {name}: body hash differs ({fname})")
            ok = False
        else:
            print(f"ok   {name} -> {fname} ({f['lines']} lines)")

    before_names = sorted(before)
    after_names = sorted(after)
    if before_names != after_names:
        added = sorted(set(after_names) - set(before_names))
        removed = sorted(set(before_names) - set(after_names))
        print(f"FAIL fn-name multiset: added={added} removed={removed}")
        ok = False
    else:
        print(f"ok   fn-name multiset unchanged ({len(before_names)} functions)")

    print("MOVE-CHECK:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--snapshot", metavar="REV")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()
    if args.snapshot:
        do_snapshot(args.snapshot)
    elif args.check:
        return do_check()
    else:
        ap.error("choose --snapshot REV or --check")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
