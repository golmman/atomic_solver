Use the pdf skill to extract the knowledge you find in `plans/dfpn/parallel.pdf`.
Use it to create a report which details how to build a parallel depth first proof number search in rust.
Write the report to `plans/dfpn/research.md`.

---

Use the pdf skill to extract the knowledge you find in `plans/dfpn/ghi.pdf`.
Use it to create a report which details how to fix the problem of transposition-table reuse under repetition giving unsound results for df-pn.
Write the report to `plans/dfpn/research_ghi.md`.

---

* Use the pdf skill to read these papers about depth first proof number search: `plans/dfpn/parallel.pdf`, `plans/dfpn/ghi.pdf`, `plans/dfpn/epsilon.pdf`.
* create a plan for a rust implementation of df-pn+ with the ghi fix and the epsilon trick
  * the implementation replaces the currently implemented minimax search
* don't include parallelization yet
* the search has to stop after 5 seconds
* store the plan in `plans/dfpn/plan2.md`
* see test positions below


mate in 4:
rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2

mate in 3:
rnbqkbnr/ppppp1pp/5p2/7Q/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 2
rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3

mate in 2:
rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3
rnbqkbnr/ppp1p2p/3p1pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 4

mate in 1:
rnbqkbnr/ppp1pQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 4
rnbq1bnr/pppkpQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR w KQ - 2 5

win for white with exploded black king:
rnb3nr/ppp4p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQ - 0 5

Draw - only two kings remain:
4k3/8/8/8/8/8/8/4K3 w - - 0 1

---

We want to implement depth first proof number search as originally planned and replace the current minimax search.
See `plans/basics/report.md` for the reasons this failed last time and use `plans/dfpn_parallel/research.md` to implement it with parallelity.
Create an implementation plan and write it to `plans/dfpn_parallel/plan.md`.

---

buildin timeout of 5 seconds

tests

10 or less
4r1k1/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R4R1K w - - 2 19
4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/R5RK w - - 4 20

9 or less
4r2k/3p4/p1pB2p1/5p1p/7P/2N1PPP1/P1PP4/1R4RK b - - 5 20
4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21

8 or less
4r2k/3p4/2pB2p1/p4p1p/6PP/2N1PP2/P1PP4/1R4RK b - - 0 21
4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22

7 or less
4r2k/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK b - - 0 22
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23

6 or less
4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R2R2K b - - 2 23
4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24

5 or less
4r1k1/3p4/2pB2p1/6Pp/p6P/2N2P2/P1PP4/1R2R2K b - - 0 24
4r1k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R2R2K w - - 0 25

4 or less
6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25
6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26

3 or less
1R4k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 1 26
1R6/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 2 27

2 or less
6R1/3p1k2/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 3 27
6R1/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K w - - 4 28

1 or less
5R2/3p4/3Bk1p1/2p3Pp/7P/p1N2P2/P1PP4/7K b - - 5 28
5R2/3p4/3Bk1p1/6Pp/2p4P/p1N2P2/P1PP4/7K w - - 0 29

win for white with checkmated black king
8/3p4/3BkRp1/6Pp/2p4P/p1N2P2/P1PP4/7K b - - 1 29

test 2

mate in 4
rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2

mate in 3
rnbqkbnr/ppppp1pp/5p2/7Q/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 2
rnbqkbnr/ppppp2p/5pp1/7Q/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 3

mate in 2
rnbqkbnr/ppppp2p/5pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 3
rnbqkbnr/ppp1p2p/3p1pp1/3Q4/8/4P3/PPPP1PPP/RNB1KBNR w KQkq - 0 4

mate in 1
rnbqkbnr/ppp1pQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 4
rnbq1bnr/pppkpQ1p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR w KQ - 2 5

win for white with exploded black king
rnb3nr/ppp4p/3p1pp1/8/8/4P3/PPPP1PPP/RNB1KBNR b KQ - 0 5


Draw - only two kings remain
4k3/8/8/8/8/8/8/4K3 w - - 0 1
