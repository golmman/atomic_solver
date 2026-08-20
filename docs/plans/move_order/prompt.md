Write the ideas to `docs/plans/move_order/ideas.md`.

---

We want to improve the benchmark suite to increase testability for the upcoming changes to move orderings (see `docs/plans/move_order/ideas.md`).

The following positions are all won for white and are ordered from hardest to easiest to find.

```
4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20
4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/1R4RK b - - 5 20
4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21
4r2k/3p4/2pB2p1/p4p1p/6PP/2N1PP2/P1PP4/1R4RK b - - 0 21
4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22
4r2k/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK b - - 0 22
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K b - - 2 23
4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24
4r1k1/3p4/2pB2p1/6Pp/p6P/2N2P2/P1PP4/1R2R2K b - - 0 24
4r1k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R2R2K w - - 0 25
6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25
6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26
1R4k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 1 26
1R6/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 2 27
6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27
6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28
5R2/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 5 28
5R2/3p4/3Bk1p1/6Pp/2p4P/p1N2P2/P1PP4/7K w - - 0 29
```

Please create an implementation plan for the benchmark improvements and store it in `docs/plans/move_order/plan1.md`.

---

We improved the benchmark suite (`docs/plans/move_order/report1.md`) and want to implement the ideas from `docs/plans/move_order/ideas.md` step by step.

We only want to keep implementations with measurable improvements though.

Write an implementation plan for the first reasonable batch of ideas to `docs/plans/move_order/plan2.md`.

---

All the following positions are proven to be decisive outcomes (no draws) in under 20 moves.
Can we use these to improve the variety of the benchmark tests?

```
2r1k2r/2R5/b4p1p/BP1ppPpP/p3P1P1/N7/8/4K2R b Kk - 4 24
4rr1k/pp2bBp1/n1p4p/3p4/5P1P/2P1B3/PP6/R2Q1RK1 b - - 4 18
r5r1/5N1k/2p2p2/pp1p3p/3Pp3/2P1P3/P7/2bQ1R1K w - - 0 30
8/1k1p4/1P2p3/p2P1P2/P7/6p1/6K1/8 b - - 0 27
r4r2/pp2p1Bk/2p3p1/1B1pP1bp/3P3P/1PN5/2P5/5R1K w - - 1 21
1k6/8/1R6/p4p1p/P2b1PpP/6P1/8/3n3K b - - 32 49
3k3r/1p1P4/4p2P/5p2/P7/2n1P3/6K1/7R b - - 0 25
7k/7p/1pp3p1/3B4/2P5/5pPP/P7/R5K1 w - - 0 26
3r2k1/8/p7/2P2P2/8/1P1p3P/P2K2r1/6R1 w - - 1 32
r2qk2r/p2n4/6pp/1ppppp2/3P1B2/5PPP/PPP1Q1B1/R4RK1 w kq - 0 18
6k1/3p3p/4pr2/8/P2PPpp1/8/1P1B1PPn/2R2R1K b - - 3 21
r4r1k/3q1P2/pp4pp/2pp4/N4Q2/2P3PP/PP2p3/R4R1K w - - 0 25
5k2/p1Rb1P2/6r1/1P2p3/P2pP1np/3P3B/3b4/3K2R1 b - - 0 28
q3kbnr/2p3pp/5p2/4p3/6b1/2PpPP2/1P1P2PP/RNBQKB1R w KQk - 0 10
8/p7/7k/3p4/1P6/3P4/P6p/4K2R b - - 0 29
3r3k/2rB3P/p7/P4p2/1p3Pp1/1P4P1/2p1p3/2R1R2K b - - 5 41
7k/pr6/4B1p1/1P6/3PpPPp/4P3/P6P/4KR2 w - - 1 28
```

---

We want to prepare for an external optimizer to finetune the `config.toml` values via gradient descent.

I suppose we need at least one quick benchmark for rapid testing and one thorough benchmark for validations.
Also we should aim for consistent benchmark output with relevant metrics.

Help me brainstorm what else we can do to provide a proper interface to the optimizer.

---

All the following positions are proven to be decisive outcomes (no draws) in under 20 moves.
Can we use these to improve the variety of the benchmark tests?

```
rnbBk2r/pp4pp/2p5/4Np2/1b1pP3/2N4P/1PP2P2/3QKB1R b Kkq e3 0 12
r4rk1/1p5p/2p3p1/p2p2R1/7P/4PP2/2P5/5K2 b - - 3 20
rnb1k2r/p2p3p/4ppp1/qp6/3PPQ2/8/PPP2PPP/R3KB1R w KQkq - 2 11
2b2k1r/pp5p/2ppp3/8/4P2P/qPPP4/P1Q2b2/5R1K w - - 0 23
rn1q3r/ppNk2pp/2p2p2/2bpp3/P2PP1b1/2P2P2/1P4PP/R1BQKB1R w KQ - 1 12
rnbq1rk1/pp4pp/2ppp3/5pP1/3P3P/BPN1PP2/P1Q5/R1nK1B1R b - - 0 14
3r3r/1p3k1p/4p1pB/4Pp2/8/8/PPP2PPP/R4RK1 w - - 0 16
r1bq1k1r/ppN4p/n1p1p3/3p1n1P/1b1P2P1/2P5/PP6/RNBQKB1R w KQ - 1 14
r1bq1k1r/ppN5/n1p1p1pp/3p1p1P/3P4/P1PB4/1P3PP1/R1BQK2R w KQ - 0 15
r3qrk1/p2pp2p/b1n2pp1/1B5P/1p1PP1P1/8/PPPQ4/R1B2RK1 w - - 0 15
r3k1r1/1b1n3p/3p1pPb/pp1Pp3/P3P3/4B3/1PP4P/R3KBR1 w Qq - 1 20
2kr1b1r/ppp3pp/4bp2/4p3/2B1P3/2PP2PP/PP1n1P2/RN1Q1R1K w - - 3 14
r1bqk2r/pp4pp/2p2p1n/1P4B1/4P3/2Pp1PP1/7P/R2QKBNR w KQkq - 0 14
r2k1bnr/p7/1p1p1ppp/1B6/4P3/2P3P1/PP3P1P/RNBQK2R b KQ - 0 10
1r2kb1r/7p/b5p1/1BP1p3/3P1p2/P3P1P1/1p3P1P/1R2K2R b Kk - 2 20
r1b2k1r/p1p4p/1pnNppp1/1B1p4/1b1PP3/2P5/PP4PP/RNB1K2R w KQ - 0 13
r2qkbnr/4p1pp/p1n2p2/1B6/3P4/8/1PP2P1P/R1BQK2R w KQkq - 0 10
rnbqk2r/4n3/2p3pp/pp1p1p1B/3PP3/6PN/PPPB3P/R2Q1RK1 w kq - 1 12
4kb1r/6pp/B1p2p2/2P5/1p4b1/BP3N2/P5PP/R3K2R b KQk - 0 17
8/5k2/2n4p/5p2/2pPpP2/6P1/2P2N1P/6K1 b - - 1 30
r1bqk1nr/p7/1p4pp/P2ppp2/3PPP2/B5PN/7P/R2QKB1R w KQkq - 0 14
5r2/7k/8/Pp1p3p/1P1PP1p1/5p1P/2P5/R1b2R1K w - - 0 24
rnb1k2r/pppp1p1p/5npb/4p3/4PP2/N7/PPPP2PP/R1BQKB1R b KQkq - 0 6
rnbqkbnr/8/6pp/pppppp1B/3PPP2/N5PN/PPP4P/R1BQ1RK1 w kq - 0 10
4k2r/6p1/2p2p2/3ppn2/1b5p/1P2P1P1/P1PP1P1P/R1B2R1K b k - 0 17
5rk1/2p4p/1p1p4/3Pp3/P3P1p1/2P2P2/1P5P/R3K1R1 w Q - 0 18
2r1k1r1/p6p/4pp2/1N1p4/3PP1P1/1p6/PP3P1P/2R1K2R w K - 0 16
2R2r1k/r7/2p2np1/p2p3p/3P1PP1/4p3/P1P4P/4N1K1 w - - 4 23
```

