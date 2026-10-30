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
