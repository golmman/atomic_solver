Make a list of improvements ordered from lowest to highest effort which would make the solver faster. Also give an indication about the speed impact for each item.

---

Write your analysis to `docs/plans/speed/analysis.md`
Then create 12 implementation plans for the first 12 items in that order.
Write the plans to `docs/plans/speed/` and store them as `plan1.md`, `plan2.md`, and so on.

---

Analyze the implementation reports in `docs/plans/speed/` and summarize the outcome.
The goal remains: to find the outcome of a position as fast as possible.
List the recommended next steps.



"6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
"rnbqkbnr/ppppp1pp/5p2/8/8/4P3/PPPP1PPP/RNBQKBNR w KQkq - 0 2"
"8/4Pk2/8/8/8/8/PP2K1p1/6R1 w - - 1 28"


---

Let's define:
```sh
fen1="6k1/3p4/2pB2p1/6Pp/7P/p1N2P2/P1PP4/1R5K b - - 0 25"
fen2="6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"
```

fen1 can be transformed into fen2 by moving c6c5.

When i search in fen2:
```sh
cargo run --release -- --fen "$fen2" --no-refine-shortest --timeout 60
```
returns almost immediatly.

When i search in fen1:
```sh
cargo run --release -- --fen "$fen1" --no-refine-shortest --timeout 60
```
times out.

I.e. for fen1 it takes at least 60 times longer than for fen2 to find a solution when they differ by only one halfmove.

Please analyze: Why is there such a big difference?
