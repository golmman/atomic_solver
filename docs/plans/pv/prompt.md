When i run `cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"`
the result is `pv: b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6`
which has two problems:

1. the pv has 11 half-moves but there is a shorter forced win with 7 half-moves.
2. b1b8 is optimal for white but then black responds with the non-optimal g8h7
   which invites b8g8 c5c4 g8g6, which would be a forced win in 5 half-moves instead of 7 half-moves.

Please analyze and the problems and propose a solution.
Write your report to `docs/plans/pv/analysis.md`



---


`cargo run --release -- --fen "4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24" --timeout 60`
produces
```
outcome: win
pv: b1b8 g8h7 e3f4 h7h8 b8c8 h8g8 e1e8 g8f7 a2a3 c6c5 c8f8 f7g7 d6e5 g7h7 f8h8
```
and exits.

h7h8 is not the strongest response for black. E.g. h7g7 or h7g8 force longer black resistance.
That means a PV is returned that is not a PPV. Also the execution stops before finding the SPPV.

With `docs/plans/pv/plan2.md` we established that only PPVs are printed, eventually converging to a SPPV.
After that we implemented the 4 plans of `docs/plans/ultimattt/`.

So either the PPV implementation was wrong in the first place or we introduced a regression later.
Please investigate.

---

```
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/6Pp/p4p1P/2N1PP2/P1PP4/1R2R2K w - - 0 24" --timeout 60 --no-refine-shortest
```
produces
```
outcome: win
pv: e3f4 e8e1 b1b4 c6c5 b4b8 g8f7 a2a3 c5c4 b8g8 f7e6 g8g7 e6f5 g7g6
```

e8e1 is not the strongest reponse by black, the better move a4a1 delays the forced white win by 1 mov.
So the printed pv is still not a PPV.
That means the last implementation (`docs/plans/pv/report3.md`) has not completely fixed the underlying issues.

What we want:
* the PPV/SPPV logic from `docs/plans/pv/report2.md`
* a fix of the problems analyzed in `docs/plans/speed/checkpoint1.md`
  * which resulted in the improvements from ultimattt, see reports of `docs/plans/ultimattt/`

Analyze the issue until fully understood, then come up with an implementation plan in `docs/plans/pv/plan4.md`.



