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
