When i run
```
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 10
```
i get
```
proof_tree: nodes=10916 win=5598 loss=5318 root_depth=21
```


When i run
```
cargo run --release -- --fen "4r1k1/3p4/2pB2p1/p5Pp/5p1P/2N1PP2/P1PP4/1R4RK w - - 1 23" --timeout 20
```
i get
```
proof_tree: nodes=7814 win=4100 loss=3714 root_depth=15
```

I expected that the proof-tree only grows when more time is put into the search. Why does it shrink?


==> answer: shorter solution are later found and replace the older bigger tree

---

When i run
```
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 256
```
i get a proof_tree.bin of size 191K.

When i run
```
cargo run --release -- --fen "4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22" --timeout 10 --pt-size 128
```
the proof tree limit is reached and the application exits.

Why would we reach the 128M limit when the final tree is only 191K?

---

Last time we finished at `docs/plans/proof/report2.md`.

We lowered the memory usage with only a minor performance impact.
Let's try to reduce the proof-tree memory even more.

Help me brainstorm more ideas!

**Some ideas:**

* optimize `ProofNode`
  * remove `depth`: can be calculated
  * change type of `parent` to `Option<NonZeroU32>`
  * replace `children` with first-child + next-sibling structure with types `Option<NonZeroU32>`

We don't expect more than u32::MAX nodes for the moment, as a simple safety net we could add a panic with message on overflow.




