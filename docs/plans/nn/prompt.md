1. run solver on batch of positions
2. from proof tree walk every internal node and record (position, move, subtree_size_of_resulting_child)
3. Train: input = position features, output = one scalar cost estimate per move, loss = pairwise ranking loss or MSE on log-cost
4. at inference: compute cost estimate for each legal move, sort ascending

Questions about the extraction pipeline:
1. how to get this batch of positions?
2. what exactly is recorded? position is clear. move? subtree?
3. what could be "cost"?
4. how to reduce cost of inference? we need to process millions of nodes per second.


2. in the tree i record only one or-node but all and-nodes, doesn't this result in a bias? or this there a symmetry we can exploit?
3. explain how this "pairwise ranking loss" works
4. how does a "NNUE-style incremental accumulator" work?

---

Proof of Concept - Move ordering neural network

1. Run solver on batch of positions
2. Train neural net (spec: `docs/spec/nn.md`) from proof tree data
3. At inference: compute cost estimate for each legal move, sort ascending
4. Measure against baseline

Help me refine this plan.

---

Last session we proved (`docs/plans/nn/report1.md`) that the implementation of the ideas in `docs/plans/nn/concept.md` are worth it.
Create an implementation plan for Gate 1.

---

Last session we finished with `docs/plans/nn/report2.md`.

Help me understand the next steps by answering these questions:
1. How to re-create the data with different timeouts?
2. Will there more data when we choose longer timeouts?
3. Can we add more positions so we generate more data?
4. Which data and information should we pass to the external model trainer (gate 2)?

---

Let's do this then:

* pin gap 1 and 2
  * update `docs/spec/nn.md` where necessary
  * write `docs/plans/nn/plan_external_trainer.md`: a rough proposal for an implementation for the external trainer
* ablation: draft the plan for design A in `docs/plans/nn/plan3.md`, with B hinted as escalation


---

Help me understand this better.
Here is my understanding:

* we want to train the nn with a pairwise ranking loss
* ideally we would use the real subtree size per move for this comparison
* the partially computed subtree sizes of the df-pn search are not suited for this
* instead we proved that taking the amount of work done per move is a suitable metric

Is this correct?
