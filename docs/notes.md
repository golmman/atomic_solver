I want to build a solver for atomic chess.
I already built an independent atomic move generator library (https://crates.io/crates/atomic-movegen) so there is no need to learn the rules and details of atomic chess.
Create a rough plan which outlines the components and techniques we need to build one from scratch.
Store the plan in `plans/brainstorming/plan_rough.md`.

---

I want to build a solver for atomic chess with proof number search.

I already built an independent atomic move generator library. Are there other parts of this projects that i can outsource as libraries without missing out on performance?

---

Here is my idea for a rework of the --no-refine-shortest option.

When a forced outcome is found print the outcome and the proof pv.
Find the proof-pv like so:
Among the AND-nodes choose those that result in the longest pv.
If there are multiple valid OR-nodes chose that one that results in the shortest pv.

Is this idea reasonable?

---

Help me define what a "proof-pv" is. Here is my try:

For outcomes "win" and "loss" (not "draw") we define a "proof-pv" as follows:
The defenders AND-nodes are chosen such that they maximize the pv length.
Attackers OR-nodes must result in a definitive outcome (win or loss), but must not be chosen optimally.

---

When i run `cargo run --release -- --no-refine-shortest --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"`
the result is

```
outcome: win
pv: b1b8 g8h7 b8h8 h7g7 h8h7 g7g8 h7g7 g8h8 g7g8 h8h7 g8g6
```

This is not a PPV since black (defender) responds with the non-optimal g8h7
which invites b8g8 c5c4 g8g6, which would be a forced win in 5 half-moves but the SPPV is 7 half-moves.

---

When running without additional options, e.g. `cargo run --release -- --fen "6k1/3p4/3B2p1/2p3Pp/7P/p1N2P2/P1PP4/1R5K w - - 0 26"`

These steps should be taken:

1. Search is started
2. An outcome is found
3. Inform about the outcome (not any pv yet, because it is not reliable)

- if the outcome is "draw", exit

4. Find a PPV
5. Print the PPV
6. Search for a SPPV

- actively check for shorter PPVs and print them if found
- when the first SPPV is found print it and exit

If at any point the timeout is reached, inform about it and exit

Just brainstorming though. What do you think, is this sound?

---

A "proof-pv"

Ideally this process:

- run clean df-pn, highest performance, no extra work for nice PVs
- print only the outcome, since the pv is no proof-pv yet
- gradually improve the proof pv

---

1. Search for outcome → print it → exit on draw.
2. Run one refinement pass that:
   • first finds a PPV,
   • then keeps finding shorter PPVs,
   • prints each improved line as it is proven,
   • stops when the SPPV is reached.
3. On timeout, print a timeout notice and exit.

This is clean, streams results as soon as they are known, and never prints an unverified PV.

---

- TOOD: restart search with limited depth
- DONE: movegen 2.0
- TODO: more test positions
- DONE: review correctness
- DONE: update docs
- DONE: eval: win, not win
- TODO: ideas from https://github.com/nelhage/ultimattt
  - DONE: extracted
  - TODO: apply
- DONE: cli: help, docs
- TODO: cli
  - TODO: timeout option
  - TODO: no-ppv option
  - TODO: no-sppv option
  - TODO: after ppv has been found: limit max-depth for remaining search
- TODO: discrepancies 4 to 5 depth mate
- TODO: always show outcome, then refine
- TODO: benchmark
- TODO: endgame tablebases
- TODO: example which tests if a list of moves is a PPV
- TODO: docs/plans/ultimattt/plan5.md

4b2k/P1Bp1p1P/3P1P2/8/8/1p1p4/bPpP4/2B4K w - - 0 1

---

- refine-cap -> new "status"
- shorten or split AGENTS.md?
- proof-tree rework

The break condition `--refine-cap` treats a cap-cut round (unknown, may still improve)
identically to a naturally exhausted bound (proven: no shorter win).
`bounded_search` could report which of the two happened; on natural
exhaustion the PV is proven shortest and could be labelled as such (and
on cap-cut with time left, one could argue for continuing). Currently
nothing in the output distinguishes the two cases.

Is there an existing initative where we can add an implementation plan?

---

I choose the algorithmic and throughput paths.
The throughput insight could be added to the lean initative.
The algorithmic items go to the dfpn initative.
What do you think?

---

The outcome for the position tested via `make stress` is hard to find.
In `docs/plans/dfpn/` and `docs/plans/lean/` we explored ways to improve but
the result is still slow compared to non proof number search approaches like fairy-stockfish.
Is there research we missed?
Take your time to analyze and come up with ideas.
Feel free to propose new ideas we could try out in a dedicated initative.

---

Draft plans for the EWS/MOPNS reading round (could reshape child selection for repetition-dominated solving), and for the clock-pressure ordering (cheap AND-side signal).

---

- append proof db with new proof_tree.bin
- parallel
- test broken?
- ghi / repetition?
- perf
- egtb
  - how much work is spent in positions with less then or equal to 4/5/6 pieces?
- AGENTS.md prune

---

Thanks!

**Comment 1**

Shouldn't we clean up the men-count/histogram code?

**Comment 2**

Shouldn't we aim to fix the search then?
To me it looks like a major issue that the search is virtually unable to find an outcome in such a simple position.

I analyzed the position and played it out to get simpler versions.
I ran the positions via ` cargo run --release -- --fen "$FEN" --timeout 600`.

Here are my results:

8/2K5/k7/8/8/8/8/4Q3 w - - 0 1
optimal: white win in 15 half moves
nodes to first outcome: more than 755986909

8/1k1K4/8/8/8/8/8/4Q3 w - - 2 2
optimal: white win in 13 half moves
nodes to first outcome: 42682820

8/8/2k1K3/8/8/8/8/4Q3 w - - 4 3
optimal: white win in 11 half moves
nodes to first outcome: 3874308

8/5K2/8/3k4/8/8/8/4Q3 w - - 6 4
optimal: white win in 9 half moves
nodes to outcome: less than 201482

---

solve_depth_limited with hardcoded 5 second timeout

---

Let's work with informed assumptions where sufficient and formulate decision where necessary.
For the decisions compile a list options and trade offs.

---

Let's make an intermediate code review and refactoring round.
The goal is to increase the maintainability and readability.

Let's create a 3 step initiative:

- First analyze and look for
  - dead code
  - outdated/unnecessary comments
  - DRY and YAGNI
  - consistency issues
  - unnecessary coupling
  - missing tests
  - unnecessary tests
  - other code smells ...
- Next write a concise list of found issues
- Next fix these issues
  - if too much risk split it into multiple separate sessions

Never compromise on performance or correctness though.

---

I want to start an intermediate research initative. The overall goal is to refine the
search: reduced node count for finding the first decicive outcome.

Goals for plans in this initiative:

- list unsolved problems with this implementation
- look for and find new ideas (web search -> research papers etc.)
- rough ideas for quick POCs
- taking measurements

Please help me structurize the new initiative.

---

---

Use the pdf-skill to transform the following pdf-files to markdown:

- `docs/theory/deep-pns-2015/deep-pns-2015.pdf`
- `docs/theory/deep-dfpn-2017/deep-dfpn-2017.pdf`

Put the extracted files right next to their originals.

---

Evalutate the following papers:

- `docs/theory/deep-pns-2015/deep-pns-2015.md`
- `docs/theory/deep-dfpn-2017/deep-dfpn-2017.md`

Is it worth opening a new item in the research initiative for this?

---

I tend to option 2: I know it is grasping at straws, but it is worth the cheap try.

Then i'd pivot research's goal rather than close it.
E.g., re-scope from "reduce node count" to "characterize the structural floor":
A closing report that consolidates the no-go record into a single document (what the solver is locked into, and why, with evidence pointers)

---

Please search the code and initiatives and tell me

- how are repetition handled?
- what impact do repetitions have on the search algorithm?
- is GHI implemented?
- are there open questions to solve?

---

epsilon 0.375

---

I want to profile the application and improve raw performance, i.e. nodes per second.
Is this worth a new initiative?

---

Sounds good to me.

Here are my answers:

1. We prepare for the cluster. For now our testing hardware is limited to the capabilities of this sandbox. The sandboxes limits can be increased to a max of ~10 cpu cores, 32G ram, gpu if needed. More than that is costly. Tell me if this is insufficient and what we needed instead.
2. I confirm. The ulimate metric to optimize is the wall time of finding the outcome and verifying the proof combined.
3. I fine with lots of researching, trying out, learning. In the end we have version control so little risk in breaking things permanently and our knowledge only grows until we find the right way.

---

Execute docs/plans/solve/plan1.md — prerequisite reading is the plan itself plus egtb/report1.md (instrumentation method), with research/structural_floor.md and parallel/measurements/plan2/README.md as layout/baseline references.

---

1. **Decision needed (user):** apply the RETHINK verdict to the `solve`
   initiative. Options, with the evidence each would need:
   - **Pivot to sharpness-first scoping:** re-estimate campaign width as
     "number of best-defense systems to tactical shots" (the §3 caveat);
     cheap to explore further with targeted solves (no ladder needed).
   - **Pivot to bottom-up frontier-push:** needs a new plan defining the
     push mechanism (solve-the-value-frontier-at-ply-k from the
     low-material end) — note §4's caveat that leaf probing inside
     top-down searches is measured dead, so this is an architecture
     change, not a table generation task.
   - **Close with the artifact:** the spike's bimodal work landscape,
     leaf profile, verify/find ratio, and TT sensitivity are the sized
     inputs any future revival starts from.

Last session we finished with `docs/plans/solve/report1.md`.
We came to the conclusion that we need to rethink the goals.

I'd like to pivot to bottom-up frontier-push. Let's try to define the push mechanism in this session.

---

I really like the SSFP mechanism. My favorite gradient is (b).
I am completely open for changes to the product surface.

Let me frame it in my own words, so we see if our understandings align:

- there is not one solve but many solve runs
- each solve run adds to the shared global proof
- we focus on easy proofs and disproofs first

---

Let's discuss the results, please continue in simple language.
So the results show that there is basically no tree overlap in the test positions?

---

Nice reasoning, very insightful!
I expect the overlap for positions with exactly 1 ply distance to be more considerable though and the SSFP not dead for that reason.
We could explore positions step by step with exactly one ply distance each run.

---

what do you think of this idea:
Proof and disproof side lines step by step and add them to the global proof.
That way the hard to prove main line gets narrower.

Example:

- Let's say the root split into 10 children
- Each child has always (for the sake of simplicity) 10 children
- so we can use the notation 0567 to uniquely identify the path where child 0, then child 5, then child 6, then child 7 was chosen
- now let's say from the root the child 0 is the (unknown) winning path
- so we explore 0 a while without a decicive outcome and we give up for now
- then we explore 1, which has a shallow decicive win, add it to the global proof
- then we explore 2, also hard so we put it aside for now
- 3 to 9 are easy, so we add them to the global proof
- we are left with 0 and 2 to proof
- go to 00: easy in this example, add to the global proof
- go to 01, hard -> skip
- 02 easy -> global proof
- and so on

---

The solve initiative is the umbrella and describes the ultimate goal of this project.
We can't simply close it without abandoning the project. Is this your recommendation?

---

Let's talk a bit about the collaboration model with ai agents.  
I'd like to keep sessions shortish and the model context focussed on a clearly defined goal.  
Most goals for sessions fall in two categories "create a plan" or "execute a plan".  
So each session should be self contained with a clean handoff and the ai agent should signal:

- if and when everything is in place to close the session
- options for follow up sessions, how to kick them off

---

**Point 1**

Plan 8 is too uncertain and costly in my opinion.
I can offer these limits for exceptional experiments for now:

- 16G memory
- 8 cpus
- 12h time

**Point 2**

We need to explore more options.
The solve initiative is the umbrella and describes the ultimate goal of this project.
We can't let it go dormant it without abandoning the project.

I don't mind if we find out that it would take years to solve on this hardware.
It's ok to have the limitations in sight and then work on reducing them step by step.

---

I would prefer to chop up what is currently called "plan8" into smaller runs which run well in this current sandbox and take max 6h.
Is this possible?

---

In my own words, so you can check if i understood:

The leading idea is not to build parallelity into this application,
instead have an orchestrator which runs multiple instances at once and then
combines the results into one proof.

---

what do you think of this idea:

Let's call it orchestrator-PNS.
The orchestrator starts with an initial (low) work budget for each worker.
Then it traverses the tree of positions breadth first and adds nodes to its bookkeeping tree.
Each node is assigned a pns

- 0 - the node is proven/disproven
- the work budget - the work budget was exceeded in this node
- INF - node not yet explored by a worker

It takes the first N children of the root and assigns them to the workers.
If a worker gets a proof/disproof inside the budget add it to the global proof and flag it OUTCOME.
If not flag the node EXCEEDED.
Repeat until there a no UNKNOWN nodes left.
Then increase the work budget and explore the EXCEEDED nodes again.
Repeat.

---

what if the orchestrator implemented a breadth-first PNS where it assigns the workers the nodes to be explored?

---

- pivot: database of prroven positions bradth first
- selfplay against stockfish?
  - measured in conversion initative: no-go
- pns mit attacker der eigentlich loser ist?
  - funktioniert, ist aber in der theorie aufwändiger
  - position wird immer als attacker OR ausgeführt
  - -> alle OR nodes müssen durchsucht werden
- nextn step solve?
  1. A5.4 — the coverage race: a mechanism innovation that wins the root's ~41-children coverage race (the one open architectural problem, since budget ladders were falsified as the fix). Requires
     a fresh pre-registered plan.
  2. A6 — budget-aware early-censor certificates: mechanism innovation targeting the resume/accumulation failure mode (no warm work-to-censor discount).
- solve: no-go parallel?
  - "Read parallel/report2.md, solve/report8–9, and campaign_architecture.md §11 (A5.4/A6); assess whether a
    TT-seeded race probe or an A6 certificate design justifies a fresh pre-registered plan, and write plan10.md or a closure note."

---

## Goal

Build a database of proven and disproven lines from the starting position.
Results are added to a global proof.
Can be started and stopped at will.

## Idea

Breadth-first PNS orchestrator runs this solver with N instances parallel.

## Questions

- new initiative?
- as an example or external?
- directly to db or append proof_tree.bin?

---

Thank you, let's brainstorm this a little further.

What is this database for:
I imagine a small website which allows to explore the proven and disproven lines.
The website is definitely not the scope of this project though.

We could merge proof_tree.bin files into the global proof.
But what structure / format is the global proof then?

If it is built as an example/ we should worry about the size. Maybe an external project is preferred?

---

My feedback / thoughts follow.

The website allows the user to replay the positions.
So i'd argue the database is a tree where certain nodes are no further expanded
since no decisive outcome was found yet.

What do you mean by "PG"?

SQLite sounds like the right tool, agreed.

Where the code lives, also agreed.

Where does the merger live though? If it is part of this project we need not to share the proof_tree_dump.md but only the global_proof_store.md.
When we store directly to and SQLite db the global_proof_store.md should only explain the structure of the stored data, right?

---

1. does the harvester start multiple parallel instances?
2. when i start the example does it already automatically add to the global proof?
3. where is the global proof sqlite file?
4. how is the next position selected by the harvester? breadth first?

---

I want the initiative to pivot then.

1. have we tried to give sub-positions to independent harvesters?
2. ok
3. the data/ directory would be a good place to store these.
4. why would we prove children of proven parents?
   I'd rather have "breadth first PNS", i.e.:

- search breadth first
- init unvisited nodes with 1 as proof/disproof number
- harvest the node with the lowest pn/dpn with a fixed budget, prefer nodes closer to the root
- if no decisive outcome update pn/dpn with the work done and pick the next node
- when no nodes with "1" pn/dpn remain visit the failed harvested nodes and doulbe their work

---

1. i'd argue the goals are different.
   There we wanted to get the fastest solver for the root position.
   In the `proofdb` initiative we want to build a database of proven positions, and of course we should try to be as fast as possible in harvesting. But the metric is different.

2. should be gitignored in my opinion. i suspect the databases grows and grows, i don't like conflating gigabytes of db positions with application code.
   i think the sidecar file should live here as well.

3. I'd argue initiative `proofdb` is for coverage AND proof.
   E.g. after g1f3 from the root position only 3 moves are not immediatly losing. and we encode this in the db as proof.
   4.1 agreed, we don't want this number in the db but in a sidecar file. but i'd still argue they are proof/disproof-numbers. if pn/dpn are not priority estimates what else are they?
   4.2 you're right. i'd rather see the most promising lines explored. in the end the database is used by atomic chess players who want to see if their opening ideas result in quick losses, traps, etc., of course they will want to explore the most proving lines more likely.
   4.3 agreed, that's the purpose of the sidecar file.
   4.4 ok, but let's add it as an explicit item in the initiative.

---

Let's have a checkpoint session.
I want to make sure the `proofdb` initiative reflects my intended design.

here is my top down vision of the finished initiative:

- run harvest from root with N threads
  - also i want to be able to run it from any OR-node / white-to-move position
- let it run for e.g. 24h
- stop it manually, we have e.g. 10 new proven positions, 10 new undecided positions
- run merge, which adds the 10 new proofs to the global proof tree
- hand over the updated proof tree to the website
- explore the extended proof tree in the website
- the website side is not part of this initiative

for the harvester i envision this:

- Let's say the position splits into 10 children
- Each child has always (for the sake of simplicity) 10 children
- so we can use the notation 0567 to uniquely identify the path where child 0, then child 5, then child 6, then child 7 was chosen
- let's say this is the first run ever
- from the root we see 10 children so we put them in the sidecar file with pn 1
- then we start exploring the children with a fixed (but cli-configurable) work budget
- child 0 to 7 are a decisive wins: update the sidecar file, write the shards
  - we never need to explore 0 to 7 again, job done
- child 8 and 9 exceed their budget
- we expand 8 and 9, that gives 20 new sub-children
- before we go deeper we explore the 20 new sub-children
- the run is stopped / terminated
- new shards have been added and the sidecar was updated
- intermediate work is lost but that's ok
- next run we pick up where we left off based on the sidecar file
- we only start expanding the next depth when all nodes of the current depth are currently being explored (in parallel) or were explored already
- at OR-nodes we are only interested in one proving move, if we find it we skip their siblings

the merger:

- picks up new shards and appends to the global proof tree

---

plan4 looks huge. should we split it or park some tasks as items in the initiative?

---

Plan a small plan5 in proofdb (docs-only, read-only probe allowed): decide the within-layer selection key for breadth-pns (report4 finding 3) — work-aware effective numbers vs a per-layer budget split — pre-register it, and run the second batch over the 1,417-job frontier from data/proofdb_work.json (ledger snapshot in measurements/plan4/); goal: measure whether the ladder can ever fire in-session.

---

Do we have two conflicting ideas for the outer pns deepening in `proofdb`?

1. we have the "ladder": if a position keeps failing, come back to it later and give it double the effort, then quadruple, and so on
2. we have the "expansion": if a node fails to be proven in the buget given we expand it and try to prove their children

---

I feel like we don't need the "ladder" when the "expansion" determines the next node to be explored. Why would we ever come back to harvest the parent when we could instead harvest the children?

---

Sounds good so far, we should be careful with the documentation and exploration methods though.
We are "inventing" a new algorithm by combining two principles here, right? So i feel like it is imperative to document the mechanism where/when exactly the split happens. When do we expand, when do we ladder? How did we find this rule? how can we configure the rule so we can benchmark variations of the rule?

---

Help me understand `docs/plans/proofdb/report6.md`.

1. we have the "ladder": if a position keeps failing, come back to it later and give it double the effort, then quadruple, and so on
2. we have the "expansion": if a node fails to be proven in the buget given we expand it and try to prove their children

We tried combining the two mechanisms in plan6 and measure where/when exactly the split should happen and establish a rule.

The result is that the "ladder" mechanism is never giving better results than the "expansion" mechanism. So the algorithm is pure "expansion" from now on.

Correct?

---

Amend plan10 (§4.1/§4.3/H2/H3 re-scoped: arm A re-baselined by a full in-session replay per §4.2, pilot kept as falsified pre-registration, arm B unchanged), then finish plan10: replay arm A (~35 min), run arm B + replay, gates, promote the 6 facts, union advance, merge, flip analysis, report10.md.

---

please investigate: how is `libs/Fairy-Stockfish/` achieving parallel search when we struggle to get it for this application?
