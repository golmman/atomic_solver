I want to build a solver for atomic chess.
I already built an independent atomic move generator library (https://crates.io/crates/atomic-movegen) so there is no need to learn the rules and details of atomic chess.
Create a rough plan which outlines the components and techniques we need to build one from scratch.
Store the plan in `plans/brainstorming/plan_rough.md`.

---

I want to build a solver for atomic chess with proof number search.

I already built an independent atomic move generator library. Are there other parts of this projects that i can outsource as libraries without missing out on performance?

-----

Here is my idea for a rework of the --no-refine-shortest option.

When a forced outcome is found print the outcome and the proof pv.
Find the proof-pv like so:
Among the AND-nodes choose those that result in the longest pv.
If there are multiple valid OR-nodes chose that one that results in the shortest pv.

Is this idea reasonable?



----

Help me define what a "proof-pv" is. Here is my try:

For outcomes "win" and "loss" (not "draw") we define a "proof-pv" as follows:
The defenders AND-nodes are chosen such that they maximize the pv length.
Attackers OR-nodes must result in a definitive outcome (win or loss), but must not be chosen optimally.

----

When i run `cargo run --release -- --no-refine-shortest --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"`
the result is
```
outcome: win
pv: b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6
```

This is not a PPV since black (defender) responds with the non-optimal g8h7
which invites b8g8 c5c4 g8g6, which would be a forced win in 5 half-moves but the SPPV is 7 half-moves.

----

When running without additional options, e.g. `cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"`

These steps should be taken:
1. Search is started
2. An outcome is found
3. Inform about the outcome (not any pv yet, because it is not reliable)
  * if the outcome is "draw", exit
4. Find a PPV
5. Print the PPV
6. Search for a SPPV
  * actively check for shorter PPVs and print them if found
  * when the first SPPV is found print it and exit

If at any point the timeout is reached, inform about it and exit

Just brainstorming though. What do you think, is this sound?

---

A "proof-pv" 



Ideally this process:
* run clean df-pn, highest performance, no extra work for nice PVs
* print only the outcome, since the pv is no proof-pv yet
* gradually improve the proof pv

----

  1. Search for outcome → print it → exit on draw.
  2. Run one refinement pass that:
    • first finds a PPV,
    • then keeps finding shorter PPVs,
    • prints each improved line as it is proven,
    • stops when the SPPV is reached.
  3. On timeout, print a timeout notice and exit.

This is clean, streams results as soon as they are known, and never prints an unverified PV.

---

* TOOD: restart search with limited depth
* DONE: movegen 2.0
* TODO: more test positions
* DONE: review correctness
* DONE: update docs
* DONE: eval: win, not win
* TODO: ideas from https://github.com/nelhage/ultimattt
  * DONE: extracted
  * TODO: apply
* DONE: cli: help, docs
* TODO: cli
  * TODO: timeout option
  * TODO: no-ppv option
  * TODO: no-sppv option
  * TODO: after ppv has been found: limit max-depth for remaining search
* TODO: discrepancies 4 to 5 depth mate
* TODO: always show outcome, then refine
* TODO: benchmark
* TODO: endgame tablebases
* TODO: example which tests if a list of moves is a PPV
* TODO: docs/plans/ultimattt/plan5.md


4b2k/P1Bp1p1P/3P1P2/8/8/1p1p4/bPpP4/2B4K w - - 0 1

---

* refine-cap -> new "status"
* shorten or split AGENTS.md?
* proof-tree rework


The break condition `--refine-cap` treats a cap-cut round (unknown, may still improve)
identically to a naturally exhausted bound (proven: no shorter win).
`bounded_search` could report which of the two happened; on natural
exhaustion the PV is proven shortest and could be labelled as such (and
on cap-cut with time left, one could argue for continuing). Currently
nothing in the output distinguishes the two cases.

Is there an existing initative where we can add an implementation plan?

---

I choose the algorithmic and throughput paths.
The throughput insight could be added to the lean initative.
The algorithmic items go to the dfpn initative.
What do you think?

---

The outcome for the position tested via `make stress` is hard to find.
In `docs/plans/dfpn/` and `docs/plans/lean/` we explored ways to improve but
the result is still slow compared to non proof number search approaches like fairy-stockfish.
Is there research we missed?
Take your time to analyze and come up with ideas.
Feel free to propose new ideas we could try out in a dedicated initative.

---

Draft plans for the EWS/MOPNS reading round (could reshape child selection for    repetition-dominated solving), and  for the clock-pressure ordering (cheap AND-side signal).

---

* append proof db with new proof_tree.bin
* parallel
* test broken?
* ghi / repetition?
* perf
* egtb
  * how much work is spent in positions with less then or equal to 4/5/6 pieces?


---

Thanks!

**Comment 1**

Shouldn't we clean up the men-count/histogram code?


**Comment 2**

Shouldn't we aim to fix the search then?
To me it looks like a major issue that the search is virtually unable to find an outcome in such a simple position.

I analyzed the position and played it out to get simpler versions.
I ran the positions via ` cargo run --release -- --fen "$FEN" --timeout 600`.

Here are my results:

8/2K5/k7/8/8/8/8/4Q3 w - - 0 1
optimal: white win in 15 half moves
nodes to first outcome: more than 755986909

8/1k1K4/8/8/8/8/8/4Q3 w - - 2 2
optimal: white win in 13 half moves
nodes to first outcome: 42682820

8/8/2k1K3/8/8/8/8/4Q3 w - - 4 3
optimal: white win in 11 half moves
nodes to first outcome: 3874308

8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4
optimal: white win in 9 half moves
nodes to outcome: less than 201482
