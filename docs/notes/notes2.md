Investigate and compile two tables of features/techniques:
* currently used features to make the solver more efficient
* researched and measured but rejected features

Both tables share these table headers:
* id: simple identifier like F1, F2, ...
* name: short name
* estimated impact: rough estimation of wall time factor this feature provides, number between 0 to 1 if it makes the search faster, greater than 1 if it makes it slower
* references: references to initiatives, plans, bibliography which describe the feature in depth

Examples for used features
* epsilon trick
* move order heuristics
* tt slots
* iterative work chunking

Examples for rejected features
* cicek style massive parallelity
* neural network move order

Store both tables in `docs/features.md`.

---

My initial intention for this project was to get the fastest atomic chess win-prover. With `provari` (see initiative) we found our match.

I see two paths from here:
1. Keep working on this project, adding ideas from provari
2. Start from scratch, pick the best from both atomic_solver and provari

What would be your recommendation?

---

Thanks, I follow your recommendation. 

I see three milesteones to get there:
* win-prover workspace
* interop initiative
* provari initiative

Please clarify: what would be the recommended order of tasks?

---

can we tie this together into one unified `winprover` initiave?

What i would do:
move items form `provari` and `interop` over, retire them, add an optional item for the workspace split. Also mark the `workspace` initiative as stale.

---

For the winprover initiative i want a `bench.md` which documents the performance progress compared to provari.
Pick some benchmark postions then document the baseline: provari as 1.0 and this project as a factor of the provari wall time.

each executed plan needs to document the new factor.
the ulimate goal is to our factor lower than 1 at some point.

---

`atomic-movegen` was compared a different movegen library `shakmaty`: `docs/plans/movegen/shakmaty_analysis.md`.

Please compile a "wishlist" of changes for `atomic-movegen` which would help in our project and store it as `docs/plans/movegen/feedback2.md`.
