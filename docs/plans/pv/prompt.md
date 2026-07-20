When i run `cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"`
the result is `pv: b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6`
which has two problems:

1. the pv has 11 half-moves but there is a shorter forced win with 7 half-moves.
2. b1b8 is optimal for white but then black responds with the non-optimal g8h7
   which invites b8g8 c5c4 g8g6, which would be a forced win in 5 half-moves instead of 7 half-moves.

Please analyze and the problems and propose a solution.
Write your report to `docs/plans/pv/analysis.md`
