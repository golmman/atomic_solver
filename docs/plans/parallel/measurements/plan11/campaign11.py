#!/usr/bin/env python3
"""plan11 stage-3 campaign driver: pre-registered rounds per case, one A0
race and one A1 race per round, interleaved with alternating order
(odd rounds A0-first, even rounds A1-first). Resumable per race via
races11.run_race's state-file skip. Machine-exclusive by convention.

Usage:
    nohup python3 campaign11.py > logs/campaign11.out 2>&1 &
"""

import os
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import races11  # noqa: E402

# Cheap cases first (fast H2 feedback), then the expensive ones. Round counts
# and caps are the pre-registered plan11 table (identical to plan10); dec13
# is veto-only (3 rounds).
CAMPAIGN = [
    ("dec13", 3),
    ("m22_white", 5),
    ("dec10", 5),
    ("rem12", 5),
    ("stress", 5),
    ("m20_white", 5),
]


def main():
    if not os.path.exists(races11.BIN):
        raise SystemExit(f"solver binary not found: {races11.BIN}")
    for case, rounds in CAMPAIGN:
        for rnd in range(1, rounds + 1):
            order = ["A0", "A1"] if rnd % 2 == 1 else ["A1", "A0"]
            for arm in order:
                rec, status = races11.run_race(case, arm, "", rnd)
                winner = rec["winner_salt"]
                win = next(
                    (r for r in rec["racers"] if r["salt"] == winner), None
                ) if winner is not None else None
                detail = (
                    f"winner={winner} evals={win['evals']} "
                    f"wall={win['wall_ms']}ms outcome={win['outcome']}"
                    if win
                    else "winner=none"
                )
                print(f"{rec['race']}: {status} {detail}", flush=True)
    print("CAMPAIGN DONE", flush=True)


if __name__ == "__main__":
    main()
