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

---

Let's re-evaluate the proof tree generation. Help me brainstorm potentials, risks, trade-offs.

I want to reduce the coupling and improve performance, so here is my idea:
* decouple search and proof tree generation
  * win: the problem of overflowing proof tree memory during search is solved
* the new default of the happy path is: the search is performed, a decisive outcome is found, shorter solutions are refined, **no proof tree is built yet**
* a new cli parameter accepts FEN and PV and restores the proof tree

Open questions:
* How much performance potential is there?
* Is restoration of a proof tree from a pv reasonable?

---

Understanding your suggestion (point 5.)
* "Search process RSS becomes TT-only" - what does "RSS" mean in this context?
* where are the logs stored? directly on the disk? wouldn't that be a performance bottleneck?

---

Thanks for the feedback. I don't like the event log though for the following reasoning:

One nice thing about our df-pn search is that it is memory bound.
I could in theory let it run for days on compute optimized machines and not worry about running out of RAM or disk space.
So that is a quality i like to get back by removing the coupling to proof tree generation.
The proof tree generation could then be outsourced to memory optimized machines.

So how do we find the reasonable middle ground?
Maybe another simple idea would be to dump the FEN and TT after the search and let the proof tree generation pick up from there.
What do you think?

---

I'd like to discuss the right space for the initiative first.
I think the correct initiative would be `docs/plans/proof/`, it is missing an `initiative.md` though.
My reasoning: the implementation idea has moved from the existing prove tree to picking up a tt dump but the goal remains - providing proof for a discovered outcome.


