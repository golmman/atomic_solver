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

---

new easy positions

```
rnbq1k1r/ppN4p/Bbp3p1/8/4PPPP/1QPp4/PP6/R4K1R b - - 1 16
rnb1kb2/pp1pp1p1/2p2p2/q7/8/2N5/PPPPPPPP/R1BQKB1R b KQq - 2 4
r3k2r/p1pN2pp/5q1n/3p4/1b1Pp1bP/2P1PP1B/PP6/R1BQK1R1 b Qkq - 0 15
r3kb1r/1p3ppp/2p5/4p3/2PP1P1P/4n3/2n1K2R/1Q4N1 w kq - 0 15
rnb2kRr/pppB4/3b4/3p3p/3P3P/4P3/PPP5/RNB1K3 b Q - 1 15
r2R4/3k4/1P3n2/p1pp3p/3Pp3/2P5/P5PP/R5K1 b - - 18 31
r1b1kbnr/ppp1p1pp/5p2/8/1n6/4P3/PPPP1PPP/RNBQKB1R w KQkq - 1 6
r1b1k1nr/pppp1Np1/2n4p/1B2pp2/1b5q/2N1P1P1/PPPP1P1P/R1BQK2R b KQkq - 0 7
r1bq1k2/ppN3p1/2p1p2P/6P1/1b1P1p2/8/PP3P2/R2K1B1R b - - 0 15
rnb1N1nr/pp2k1pp/B1p2q2/3p4/5PPP/4p3/PPP1K3/RNB4R b - - 3 14
7k/pp6/4prpp/3pQp2/3P1P2/4P3/PP4PP/4K2R b K - 0 23
rnbqk2r/p4pp1/7p/1pBp4/4P2N/2P2PP1/PP1Q3P/RN2nK1R b kq - 0 13
rnb1kb1r/ppp2Npp/4p3/q2p1pB1/3PP3/2P2P2/PP4PP/RN2KB1R w KQkq - 1 11
k2r3r/2B3pp/5p2/p3p3/1P6/4P1PP/P4P2/7K b - - 0 25
8/pp6/5ppP/6P1/kP6/3R4/P1P5/6K1 b - - 0 28
3r3k/2p4p/6p1/8/5P2/2P1P2P/1P4Pb/4K2R w K - 0 19
6k1/4r1P1/2pp1P2/8/2b4p/4R3/1PP1n2K/4R3 b - - 0 35
rn2kb1r/p1pp1pp1/8/1p5p/8/3PP1P1/PPP2n1P/R1BQ1RK1 w kq - 0 13
2kr4/p1pb1p2/3p3p/3Pp3/4P3/7P/P2QK3/6R1 b - - 0 24
1k1r1N2/3P2p1/p6r/P3pp2/7P/Bpp1P3/4b3/2K1R1R1 w - - 0 33
2kr3r/p5pp/4p2n/2pP4/8/1R6/PP3PPP/2K4R b - - 1 15
2r2r1k/2Bq3p/p4pp1/7Q/P1pPP3/1p5P/1P6/R5RK b - - 3 24
4n1k1/4P3/2p2Rp1/p2p3p/P2P4/6P1/7P/2K5 b - - 1 33
r7/1Rp4k/4P2B/6p1/3P2P1/p6p/P6K/8 b - - 0 35
1r3Q2/8/k3p1b1/4P3/P7/K7/6p1/6R1 b - - 0 34
4k2r/1p2P3/7P/p1Pp4/3P4/8/5pp1/2K2R2 w - - 0 34
```
