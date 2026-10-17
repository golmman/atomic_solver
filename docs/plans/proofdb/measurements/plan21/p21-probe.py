#!/usr/bin/env python3
"""plan21 probes: or-closed structure over the standing layers.

Read-only: reconstructs node paths from the parent chain and counts, per
DB, (a) proven `loss` rows at odd absolute ply, (b) distinct or-closed
parents (white-to-move, even ply), (c) the number of censor records that
the plan21 static drop would remove (at-or-under an or-closed parent),
and (d) the total number of nodes at-or-under an or-closed parent (the
upper bound of future drops).

Usage: p21-probe.py <db> [<db> ...]  — writes one JSON line per DB.
"""
import json
import sqlite3
import sys


def probe(db_path):
    c = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    parent = {r[0]: r[1] for r in c.execute("SELECT id, parent_id FROM nodes")}
    mv = {r[0]: r[1] for r in c.execute("SELECT id, move_uci FROM nodes")}

    def path(i):
        p = []
        while i is not None:
            m = mv[i]
            if m:
                p.append(m)
            i = parent[i]
        return list(reversed(p))

    P = {i: path(i) for i in mv}
    orc = set()
    loss_rows = 0
    for (i,) in c.execute("SELECT id FROM nodes WHERE outcome='loss' AND ply%2=1"):
        loss_rows += 1
        parent_path = P[i][:-1]
        if len(parent_path) % 2 == 0:
            orc.add(" ".join(parent_path))
    recs = [r[0] for r in c.execute("SELECT path FROM censor")]
    dropped = [p for p in recs if any(p == o or p.startswith(o + " ") for o in orc)]
    under = sum(
        1
        for i in P
        if any((s := " ".join(P[i])) == o or s.startswith(o + " ") for o in orc)
    )
    c.close()
    return {
        "db": db_path,
        "nodes": len(P),
        "loss_rows_odd_ply": loss_rows,
        "or_closed_parents": len(orc),
        "censor_records": len(recs),
        "or_closed_dropped_records": len(dropped),
        "nodes_under_or_closed": under,
    }


for p in sys.argv[1:]:
    print(json.dumps(probe(p)))
