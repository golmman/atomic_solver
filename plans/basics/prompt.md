Goal: Build a pure atomic-chess solver from scratch, leveraging the existing `atomic-movegen` crate for move generation and position representation. The solver determines the exact game-theoretic outcome of a position (win / loss / draw) and, when the outcome is a forced win or loss, produces a principal variation.

Create an implementation plan for the first set of features:
* `atomic-movegen` crate for move generation
* depth-first proof-number search with iterative deepening
* starting position can be passed as a FEN-string, defaults to the default chess starting position
* simple output: depth, nodes searched, nodes per second

Ask for clarification if necessary.

The plan should include a final task that documents the things learned during implementation (problems, surprises, workarounds etc.) in a file `plans/basics/report.md`.
Write the plan to `plans/basics/plan.md`
