#!/usr/bin/env python3
"""Build the standing shard-directory manifest (`docs/plans/proofdb/shards/
manifest.json`) from plan1's conforming seed manifest (`manifest_seed.json`).

Adapted from `measurements/plan1/build_seed_manifest.py` for the new
location (plan2 D1): the seed entries are already conforming to the
`global_proof_store.md` manifest contract and already carry the
startpos-relative `moves` paths, so no path reconstruction or replay
verification is needed here — the merger re-checks every entry by Zobrist
hash at merge time anyway.

The standing manifest keeps the **required fields only** (`tag`, `file`,
`fen`, `moves`, `outcome`, `validate`): it is the file the Rust
`proofdb::write_manifest` writer rewrites after each harvest batch, so the
seed entries must live in exactly that format. The seed provenance extras
(`tier`, `dtm_length`, `tree_nodes`, `path_source`, `replay_fen`) stay
preserved in plan1's `manifest_seed.json`; they are not part of the
manifest contract and are intentionally dropped here.

The `shards` table of a DB built from this manifest carries the seed
shards' own root claims; the per-tag provenance extras for the seed remain
reproducible from plan1. Output bytes are deterministic (fixed field
order, entries in plan1 file order — the merger sorts by tag itself).

Usage: python3 build_standing_manifest.py
Writes ../../shards/manifest.json next to this script's directory layout.
"""

from __future__ import annotations

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
SHARDS = os.path.abspath(os.path.join(HERE, "..", "..", "shards"))
SEED_MANIFEST = os.path.join(
    HERE, "..", "plan1", "manifest_seed.json"
)

REQUIRED = ("tag", "file", "fen", "moves", "outcome", "validate")


def main() -> None:
    with open(SEED_MANIFEST) as f:
        seed = json.load(f)
    entries = [{k: e[k] for k in REQUIRED} for e in seed["entries"]]
    tags = [e["tag"] for e in entries]
    assert len(set(tags)) == len(tags), "duplicate seed tags"
    paths = [" ".join(e["moves"]) for e in entries]
    assert len(set(paths)) == len(paths), "duplicate seed paths"
    assert all(e["validate"] == "ok" for e in entries)
    # Every referenced shard file must exist in the standing directory.
    missing = [e["file"] for e in entries
               if not os.path.exists(os.path.join(SHARDS, e["file"]))]
    assert not missing, f"missing shard files: {missing}"

    out = {
        "entries": entries,
        "summary": {
            "n_entries": len(entries),
            "source": "docs/plans/proofdb/measurements/plan1/manifest_seed.json",
            "extras_dropped": True,
        },
    }
    out_path = os.path.join(SHARDS, "manifest.json")
    with open(out_path, "w") as f:
        json.dump(out, f, indent=1)
    print(json.dumps(out["summary"], indent=1))
    print(f"wrote {out_path}")


if __name__ == "__main__":
    main()
