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

When i run `cargo run --release -- --no-refine-shortest --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"`
the result is `pv: b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6`
and a win for white.

In my understanding this pv is not proof of a forced win since black responds with the non-optimal g8h7
which invites b8g8 c5c4 g8g6, which would be a forced win in 5 half-moves instead of the optimal 7 half-moves.

Note that this was already analyzed in `docs/plans/pv/analysis.md`.

For the given (not necessarily best) sequence of white moves
a proof pv would include the strongest defence for black.

When a definitive outcome is found a proof pv by the definition above should be available without additional search.

Are my assumptions correct? Is my reasoning sound?
Don't implement anything yet, this is just a brainstorming session.

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
* TODO: discrepancies 4 to 5 depth mate
* TODO: always show outcome, then refine
* TODO: benchmark

4b2k/P1Bp1p1P/3P1P2/8/8/1p1p4/bPpP4/2B4K w - - 0 1



