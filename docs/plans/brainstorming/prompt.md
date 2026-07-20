I want to build a solver for atomic chess.
I already built an independent atomic move generator library (https://crates.io/crates/atomic-movegen) so there is no need to learn the rules and details of atomic chess.
Create a rough plan which outlines the components and techniques we need to build one from scratch.
Store the plan in `plans/brainstorming/plan_rough.md`.

---

I want to build a solver for atomic chess with proof number search.

I already built an independent atomic move generator library. Are there other parts of this projects that i can outsource as libraries without missing out on performance?

-----

* TOOD: restart search with limited depth
* DONE: movegen 2.0
* TODO: more test positions
* DONE: review correctness
* DONE: update docs
* DONE: eval: win, not win
* TODO: ideas from https://github.com/nelhage/ultimattt
  * DONE: extracted
  * TODO: apply


4b2k/P1Bp1p1P/3P1P2/8/8/1p1p4/bPpP4/2B4K w - - 0 1



