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
* discrepancies 4 to 5 depth mate
* always show outcome, then refine
* benchmark

4b2k/P1Bp1p1P/3P1P2/8/8/1p1p4/bPpP4/2B4K w - - 0 1



