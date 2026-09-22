#!/usr/bin/env python3
"""Solved-set store (S) v0 — merge and export (`solve` initiative plan2).

The SSFP campaign maintains a persistent, disk-backed solved set S: exact
Zobrist keys (position + halfmove clock, the TT's own key semantics) with
WDL values. v0 is a plain binary store produced by this merge tool; no
server, no database (plan2 §2).

# Soundness classes (plan2 §2, normative)

- **verified**: keys/values extracted only from replay-validated proof
  trees (`reconstruct_pt` `validate: ok`). These compose into the master
  artifact.
- **provisional**: solved TT-snapshot records not covered by a validated
  tree (search-semantics proven, not independently verified). Usable as
  in-campaign anchors; **excluded from artifact composition** unless later
  covered by a validated tree.

Repetition discipline is inherited from the TT snapshot contract:
path-independent records only (repetition-dependent results are never
stored — first-player-loss GHI shortcut). Keys are exact (clock included),
so anchor reuse is exact-key only — sound by construction.

# Store format v0 (little-endian, packed)

    offset  size  field
    0       8     magic = "ATOMSSTO"
    8       1     version = 1
    9       1     flags = 0
    10      n     label line, UTF-8, '\n'-terminated (provenance note)
    +0      8     verified_count: u64
    +8      8     provisional_count: u64
    verified records × verified_count      (15 bytes each, the TT snapshot
    provisional records × provisional_count  solved-record layout)
      key: u64, outcome: u8 (draw=0/win=1/loss=2), depth: u32, best_move: u16
      (0xFFFF = Move::NONE)

Records are sorted by key within each class; keys are deduped per class.
A key present in both classes is stored verified (provisional is dropped).

# Export

`export` writes the store's records (both classes: both are usable as
in-campaign anchors) as a TT-snapshot-format solved section, loadable by
the solver's `--tt-load-path`.
"""

from __future__ import annotations

import argparse
import json
import os
import struct
import sys

MAGIC = b"ATOMSSTO"
VERSION = 1
OUTCOME_TO_U8 = {"draw": 0, "win": 1, "loss": 2}
U8_TO_OUTCOME = {v: k for k, v in OUTCOME_TO_U8.items()}
MOVE_NONE_BITS = 0xFFFF
RECORD = struct.Struct("<QBIH")


class SStore:
    def __init__(self, label=""):
        self.label = label
        self.verified = {}  # key -> (outcome, depth, best_move)
        self.provisional = {}  # key -> (outcome, depth, best_move)
        self.conflicts = 0  # verified/provisional outcome disagreements

    # -- ingestion ---------------------------------------------------------

    def add_verified(self, records, source=""):
        self._merge(self.verified, records, source)

    def add_provisional(self, records, source=""):
        self._merge(self.provisional, records, source)
        # Verified coverage beats provisional: keys already verified are not
        # stored provisional.
        for key in list(self.provisional):
            if key in self.verified:
                del self.provisional[key]

    def _merge(self, into, records, source):
        added = dropped = 0
        for r in records:
            key = int(r["key"])
            val = (
                r["outcome"],
                int(r["depth"]),
                int(r["best_move"]),
            )
            old = into.get(key)
            if old is None:
                into[key] = val
                added += 1
            elif old[0] != val[0]:
                # Outcome disagreement inside one class: first-wins, counted.
                dropped += 1
            else:
                dropped += 1
        if dropped:
            print(
                f"sstore: {source}: {added} added, {dropped} duplicate/conflict "
                f"records dropped",
                file=sys.stderr,
            )
        return dropped

    def reconcile(self):
        """Drop provisional entries covered by verified; count outcome
        disagreements between the classes (verified wins — and a
        disagreement is an anomaly worth surfacing, never silently fatal
        for the provisional class)."""
        for key in list(self.provisional):
            if key in self.verified:
                if self.provisional[key][0] != self.verified[key][0]:
                    self.conflicts += 1
                del self.provisional[key]
        return self.conflicts

    # -- persistence -------------------------------------------------------

    def save(self, path):
        self.reconcile()
        v = [
            (k, *self.verified[k])
            for k in sorted(self.verified)
            if self.verified[k][0] in OUTCOME_TO_U8
        ]
        p = [
            (k, *self.provisional[k])
            for k in sorted(self.provisional)
            if self.provisional[k][0] in OUTCOME_TO_U8
        ]
        with open(path, "wb") as f:
            f.write(MAGIC)
            f.write(bytes([VERSION, 0]))
            f.write(self.label.encode("utf-8") + b"\n")
            f.write(struct.pack("<QQ", len(v), len(p)))
            for key, outcome, depth, mv in v:
                f.write(RECORD.pack(key, OUTCOME_TO_U8[outcome], depth, mv))
            for key, outcome, depth, mv in p:
                f.write(RECORD.pack(key, OUTCOME_TO_U8[outcome], depth, mv))

    @classmethod
    def load(cls, path):
        with open(path, "rb") as f:
            data = f.read()
        if data[:8] != MAGIC:
            raise ValueError(f"{path}: bad magic")
        version, flags = data[8], data[9]
        if version != VERSION or flags != 0:
            raise ValueError(f"{path}: unsupported version/flags {version}/{flags}")
        nl = data.index(b"\n", 10)
        label = data[10:nl].decode("utf-8")
        off = nl + 1
        v_count, p_count = struct.unpack_from("<QQ", data, off)
        off += 16
        store = cls(label)
        for _ in range(v_count):
            key, ob, depth, mv = RECORD.unpack_from(data, off)
            off += RECORD.size
            store.verified[key] = (U8_TO_OUTCOME[ob], depth, mv)
        for _ in range(p_count):
            key, ob, depth, mv = RECORD.unpack_from(data, off)
            off += RECORD.size
            store.provisional[key] = (U8_TO_OUTCOME[ob], depth, mv)
        if off != len(data):
            raise ValueError(f"{path}: trailing bytes")
        return store

    # -- export ------------------------------------------------------------

    def export_snapshot(self, path, root_fen):
        """Write the store (both classes) as a TT-snapshot v1 file with an
        empty unsolved section, loadable via `--tt-load-path`."""
        self.reconcile()
        records = sorted(
            [(k, *self.verified[k]) for k in self.verified]
            + [(k, *self.provisional[k]) for k in self.provisional]
        )
        with open(path, "wb") as f:
            f.write(b"ATOMTTSN")
            f.write(bytes([1, 0]))
            f.write(root_fen.encode("utf-8") + b"\n")
            f.write(struct.pack("<IIQQ", 0, 0, len(records), 0))
            for key, outcome, depth, mv in records:
                f.write(RECORD.pack(key, OUTCOME_TO_U8[outcome], depth, mv))
        return len(records)

    def stats(self):
        return {
            "label": self.label,
            "verified": len(self.verified),
            "provisional": len(self.provisional),
            "total": len(self.verified) + len(self.provisional),
            "conflicts": self.conflicts,
        }


def _records_from_snapshot(path):
    """Provisional-class records: a snapshot's whole solved section."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import ssfp

    _header, solved, _unsolved = ssfp.read_tt_snapshot(path)
    return solved


def _records_from_tree(tree_path):
    """Verified-class records from a validated proof-tree dump."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import ssfp

    return ssfp.verified_records_from_tree(tree_path)


# -------------------------------- selftest ----------------------------------


def cmd_selftest(_args):
    """Unit tests: provenance class separation, dedup, conflict resolution,
    round-trip, and export format validity."""
    import tempfile

    tmp = tempfile.mkdtemp(prefix="sstore_test_")
    store = SStore(label="selftest")

    # Provenance class separation: verified stays, provisional is excluded
    # from the verified class and dropped when covered by verified.
    store.add_verified(
        [{"key": 1, "outcome": "win", "depth": 3, "best_move": 7}], "v1"
    )
    store.add_provisional(
        [{"key": 1, "outcome": "win", "depth": 3, "best_move": 7}], "p-covered"
    )
    store.add_provisional(
        [{"key": 2, "outcome": "loss", "depth": 5, "best_move": 0xFFFF}], "p-open"
    )
    store.reconcile()
    assert 1 in store.verified and 1 not in store.provisional
    assert 2 in store.provisional and 2 not in store.verified

    # Outcome conflict: verified wins, conflict counted.
    store.add_provisional(
        [{"key": 3, "outcome": "draw", "depth": 1, "best_move": 0}], "p-conflict"
    )
    store.add_verified(
        [{"key": 3, "outcome": "win", "depth": 2, "best_move": 9}], "v-conflict"
    )
    store.reconcile()
    assert store.verified[3][0] == "win"
    assert store.conflicts == 1

    # Round-trip through the binary format.
    sp = os.path.join(tmp, "s.bin")
    store.save(sp)
    loaded = SStore.load(sp)
    assert loaded.verified == store.verified
    assert loaded.provisional == store.provisional
    assert loaded.label == "selftest"

    # Export is a valid TT-snapshot v1 solved section (strict reader accepts,
    # record count matches, unsolved section empty).
    exp = os.path.join(tmp, "export.tt")
    n = loaded.export_snapshot(exp, "test fen 0 1")
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import ssfp

    header, solved, unsolved = ssfp.read_tt_snapshot(exp)
    assert n == len(store.verified) + len(store.provisional)
    assert header["solved_count"] == n and len(solved) == n
    assert header["unsolved_count"] == 0 and not unsolved
    by_key = {r["key"]: r for r in solved}
    assert by_key[1]["outcome"] == "win" and by_key[1]["best_move"] == 7
    assert by_key[2]["outcome"] == "loss" and by_key[2]["best_move"] == 0xFFFF

    print("sstore selftest: ok")
    print(json.dumps(loaded.stats(), indent=1))


# --------------------------------- cli --------------------------------------


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("merge", help="merge sources into a store")
    p.add_argument("--store", required=True, help="store file to create/update")
    p.add_argument("--label", default="", help="provenance label line")
    p.add_argument("--snapshot", action="append", default=[], help="provisional-class input (TT snapshot solved section); repeatable")
    p.add_argument("--tree", action="append", default=[], help="verified-class input (validated proof-tree dump); repeatable")
    p.set_defaults(fn=cmd_merge)

    p = sub.add_parser("export", help="export the store as a --tt-load-path snapshot")
    p.add_argument("--store", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--root-fen", required=True)
    p.set_defaults(fn=cmd_export)

    p = sub.add_parser("stats", help="print store statistics")
    p.add_argument("--store", required=True)
    p.set_defaults(fn=cmd_stats)

    sub.add_parser("selftest").set_defaults(fn=cmd_selftest)

    args = ap.parse_args()
    args.fn(args)


def cmd_merge(args):
    store = SStore(label=args.label) if not os.path.exists(args.store) else SStore.load(args.store)
    if args.label:
        store.label = args.label
    for snap in args.snapshot:
        recs = _records_from_snapshot(snap)
        store.add_provisional(recs, source=os.path.basename(snap))
    for tree in args.tree:
        recs = _records_from_tree(tree)
        store.add_verified(recs, source=os.path.basename(tree))
    store.save(args.store)
    print(json.dumps(store.stats(), indent=1))


def cmd_export(args):
    store = SStore.load(args.store)
    n = store.export_snapshot(args.out, args.root_fen)
    print(f"exported {n} records to {args.out}")


def cmd_stats(args):
    store = SStore.load(args.store)
    print(json.dumps(store.stats(), indent=1))


if __name__ == "__main__":
    main()
