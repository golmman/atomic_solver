# Scalable Parallel DFPN Search

**Jakub Pawlewicz and Ryan B. Hayward**

> Extracted from `spdfpn-2014.pdf` (author copy, Hayward's site
> webdocs.cs.ualberta.ca/~hayward/papers/pawlhayw.pdf; automated text
> extraction with pypdf, ligatures normalized; figures, tables, and
> display equations are lossy — the PDF is authoritative). Published as:
> Pawlewicz, J., Hayward, R.B.: Scalable Parallel DFPN Search. In: van den
> Herik, H.J., Iida, H., Plaat, A. (eds.) Computers and Games (CG 2013),
> LNCS 8427, pp. 138–150. Springer (2014). DOI: 10.1007/978-3-319-09165-5_12.
> **No arXiv version exists** — the arXiv:1503.07698 pointer once recorded
> in `docs/bibliography.md` resolves to an unrelated XENON1T paper and has
> been removed from that entry.
> Mined in: `docs/plans/parallel/research_spdfpn.md` (the SPDFPN
> mechanism paper behind Solrex's concurrency model; option-A verdict
> under accepted nondeterminism).

<!-- page 1 -->

Scalable Parallel DFPN Search
Jakub Pawlewicz1 and Ryan B. Hayward2
1 Institute of Informatics, University of Warsaw,pan@mimuw.edu.pl
2 Computing Science, University of Alberta,hayward@ualberta.ca
H.J. van den Herik et al. (Eds.): CG 2013, LNCS 8427, pp. 138-150, 2014.
DOI:10.1007/978-3-319-09165-5_12, c© Springer International Publishing Switzerland 2014
Abstract. We present Scalable Parallel Depth-First Proof Number Search, a new
shared-memory parallel version of depth-first proof number search. Based on the
serial DFPN 1+ε method of Pawlewicz and Lew, SPDFPN searches effectively
even as the transposition table becomes almost full, and so can solve large prob-
lems. To assign jobs to threads, SPDFPN uses proof and disproof numb ers and
two parameters. SPDFPN uses no domain-specific knowledge or heuristics, so it
can be used in any domain. Our experiments show that SPDFPN scales well and
performs well on hard problems.
We tested SPDFPN on problems from the game of Hex. On a 24-core mach ine
and a 4.2-hour single-thread task, parallel efficiency ranges from 0.8 on 4 threads
to 0.74 on 16 threads. SPDFPN solved all previously intractable 9×9 Hex open-
ing moves; the hardest opening took 111 days. Also, in 63 days, it solve d one
10×10 Hex opening move. This is the first time a computer or human has solved
a 10×10 Hex opening move.
1 Introduction
Depth-First Proof-Number search is effective for solving p roblems — e.g. two-player
games — that can be modelled with an and-or tree. It is especia lly effective on trees
with non-uniform branching, and has been successful in checkers [23], shogi [9], tsume-
shogi [12], Go [13, 14, 25], and Hex [8, 1].
DFPN search often jumps around the tree. An effective parallel DFPN variant thus
needs a shared transposition table (tt). The starting point for Scalable Parallel DFPN
search, our shared-memory DFPN variant, is the serial 1 +ε-method [19], which is ef-
fective when search space exceeds memory.
Other parallel variants based on proof numbers have been pro posed. Nagai intro-
duced proof-disproof search [16] and Kishimoto gave a paral lel version [11]. Saffi-
dine et al. proposed a job-level PN2 [21] which is effective when search space exceeds
memory, although nodes can be recomputed many times. Kanekoachieves a parallel ef-
ficiency of 0.5 with 8 threads in tsume-shogi [9], whereas SPDFPN achieves 0.7 in Hex.
Hex solvers tend to obtain better speedups than tsume-shogi solvers, perhaps because
Hex has a larger branching factor and so smaller node expansion rate.
Saito et al. introduced randomized parallel PNS [22]. Wu et a l. [24] use PNS with
virtual proof and disproof numbers and a time-intensive lea f initiation in a tree that is
small enough to stay in memory. SPDFPN is inspired by these vi rtual numbers, but is

<!-- page 2 -->
based on DFPN rather than PNS. It distributes work among thre ads using only proof
and disproof numbers and thus needs no game-specific heuristic.
We tested SPDFPN on two sets of Hex problems, including all th irteen previously
intractable 9 ×9 opening moves. See Figure 1. Experiments show that our algo rithm
scales well up to at least 16 threads.
2 DFPN
2.1 PN Search
Proof number search maintains a tree in which each node — corr esponding to a game
position — has a proof number (pn) and disproof number (dn). A node’s (dis)proof
number is the smallest number of leaves that, if all true (fal se), would make the node
true (false). Thus for an or-node p = min j pj and d = ∑
j dj, where p (d) is the
node’s (dis)proof number, and pj (dj) the j’th child’s (dis)proof number. PN search
relies on the existence of a most-proving node (mpn): this no de’s (dis)proof reduces
either the proof or disproof number. PN search iteratively s elects a most-proving leaf
and expands it. PN search needs the whole search tree to be in memory. This restriction
can be mitigated by taking a depth-first search approach and using a tt.
2.2 Depth-first PN Search
In computing (dis)proof numbers, only descendant (dis)proof values are needed. DFPN
search exploits this property by postponing ancestor updat es until the most-proving
node is no longer in the current node’s subtree. For each node, thresholds P and D are
defined so that p < P and d < D if and only if there exists a most-proving node in the
node’s subtree. These thresholds are computed recursivelyusing the following formula
for an or-node. Children are indexed in non-decreasing proo f number, e.g. p1(d1) are
the (dis)proof numbers of a child with smallest proof number . See [18] or [19] or the
recent survey on game tree search using pns [10].
P1 = min(P, 1 + p2), (1)
D1 = D − (d − d1). (2)
3 Transposition table and DAGs
To this point we have described the process for computing (dis)proof numbers in trees.
Many games allow transpositions, and so are modelled by directed acyclic graphs
(dags) rather than trees. While more complicated variants of DFPN are available for
dags [12], we find that in Hex, good results are obtained by treating the dag as a tree, as
long as a tt is used.

<!-- page 3 -->
3.1 Problems, memory and the 1 + ε method
Even with a tt, DFPN search behaves differently from the usual depth-first search, as the
continued selection of the next mpn causes frequent switchi ng among child branches.
To force DFPN to switch branches less often, one can use the1 + ε method [19], which
simply enlarges the pn threshold in (1):
P1 = min(P, ⌈(1 + ε)p2⌉).
Another problem arises in (d)pn calculations when dags are t reated as trees: due
to transpositions, (d)pns can be (exponentially) overcoun ted. Miscounting can lead to
the mistaken selection of a node which is not most-proving. T echniques address this
issue in games, such as tsume-shogi, with many transpositions [12]. We chose rather to
address these issues by starting with the 1 + ε method, which reduces branch switching
even if search space is large. For our Hex experiments, possibly miscounting (dis)proof
numbers did not cause difficulties. After trial and error, we set ε to 0. 25.
4 Parallelization
The version of PN search we have described to this point behaves like a usual depth-first
algorithm, spending significant periods of time in deep recu rsive calls. This allows for
parallelization if these criteria, which motivate the design of SPDFPN, can be met:
(i) Different threads should call DFPN searches for differe nt states.
(ii) A thread should not duplicate the work of another. (The t hreads share a tt, and
different threads might explore different strategies for the same state.)
(iii) Assignment of states to threads should follow the natu ral order of PN search.
(Assume that state A is assigned to thread α . If state B is likely to be the next
state considered, we can assign B to another thread before α finishes.)
(iv) The assignment of states to threads should take little t ime.
(v) A thread should exit its search once other thread results render it unnecessary.
4.1 Measuring work
We now show how we realize our criteria. Because of (v) we want to allow individual
threads to halt and perhaps later resume. A straightforwardattempt to parallelize DFPN
that assigns a node to a processor, and has the processor return only when the subtree is
solved, would not permit this.
To realize (v), we set a work threshold for a single DFPN call.The threshold (Max-
WorkPerJob) must be small enough to allow even distribution among threa ds, but not
so small that we lose (iv). If it is large enough we satisfy (iv) and can use more sophis-
ticated methods for state assignment among threads. We need such a threshold, since
(d)pns in a DFPN search can remain low and not reach any (d)pn t hresholds for long
periods of time, especially in sudden-death games.

<!-- page 4 -->
Implementation So how do we alter DFPN to incorporate the work threshold? In
addition to the DFPN threshold parameters P and D, we introduce a new threshold pa-
rameter W , the maximum work that a thread should perform before halting. We define
work as the number of calls to the DFPN function.
The 1+ε method works best with advanced tt collision resolution. Weuse a method
from computer chess (e.g. [15, 20]) with k = 4 : upon collision, search the next at
most k cells for an empty location; if none is found, overwrite the l ocation whose job
has performed the smallest amount of work. See [3, 17, 18] for other replacement or
collection techniques.
SPDFPN pseudocode in Algorithms 1 and 2 uses these variables: c — array of child
nodes; P, D — proof and disproof number thresholds; W — work threshold; n —
node. A node has fields .p , .d (proof and disproof numbers), .w (work), .j (index of last
selected child). TTW RITE (n) writes results for node n in the tt. TTR EAD (n, j ) tries to
read the jth child of node n; if this fails, it creates a child node with (d)pns each set to
1 and work set to 0.
Algorithm 1. DFPN Search
1: function DFPN( n, P, D, W)
2: wlocal← 1, n.w← n.w + 1
3: for each child j of n do
4: c[j]← TTR EAD (n, j)
5: loop
6: PNU PDATE(n, c)
7: TTW RITE (n)
8: if n.p≥ P∨ n.d≥ D then
9: return wlocal
10: if wlocal≥ W then
11: return wlocal
12: j, Pj, Dj← SELECT (n, c, P, D)
13: wchild←
14: DFPN (c[j], Pj, Dj, W− wlocal)
15: wlocal← wlocal + wchild
16: n.w← n.w + wchild
DFPN returns the amount of work done locally, i.e. in this cal l and all recursive
calls. S ELECT returns a child together with thresholds for recursive call . In the single-
threaded version of the 1 + ε method, processing remains at a node n1 until its pn
p1 exceeds the second-smallest pn p2 (among siblings) by a ratio of 1 +ε. Since we
allow processes to be interrupted, it can be that upon resump tion p1 is larger than p2
but smaller than p2(1 + ε). In this case, processing should resume at n1 rather than its
sibling. This requires adding to function SELECT the if-statement in lines 6–11.
5 Work assignment
What are a thread’s candidates for state assignment? PN-Search descends through a path
from root to a most-proving node. In DFPN this path is created by successive recursive
calls and so is stored on a stack. However, DFPN search can remain deep in the search
tree for long periods. Thus for a node whose path is close to th e most-proving node,
DFPN can stay in a subtree of that node for some time. Such a node is a good candidate
for a thread state assignment. But how deep on the subtree pat h should the assigned
state be? It should be deep enough that DFPN will stay in subtr ee of the node, not too

<!-- page 5 -->
shallow because of (v) and not too deep because of (iv). A goodcandidate is a node that
is closest to the root and with past work performed belowMaxWorkPerJob, because we
expect that total DFPN work for this node will be proportiona l to the total DFPN past
work.
Once we assign a state A to a thread α , how should we assign a state B to another
thread? If we follow the same procedure we would arrive at the same state. Instead,
following Rémi Coulom (see [4, p. 64]), we temporarily assign a virtual w in or loss
to A until α finishes its search. This idea is also used by Job Level PN sear ch, which
achieves superlinear scalability, and fulfils (i), (ii) and (iii). See [24].
Algorithm 2. DFPN Search — utility functions for OR node
1: procedure PNU PDATE(n, c)
2: n.p← min
child j of n
c[j].p
3: n.d←
∑
child j of n
c[j].d
4: function SELECT (n, c, P, D)
5: j1← child with the smallest pn
6: if n.j is set and n.j⁄= j1 then
7: ⊲ Try continue with the same child
8: Pn.j, Dn.j←
9: T HRESHOLDS (n, c, P, D, n.j, j1)
10: if c[n.j].p < P n.j then
11: return n.j, Pn.j, Dn.j
12: n.j← j1
13: j2← child with the second smallest pn
14: Pj1 , Dj1←
15: T HRESHOLDS (n, c, P, D, j1, j2)
16: return j1, Pj1 , Dj1
17: function THRESHOLDS (n, c, P, D, j1, j2)
18: Pj1← min(P,⌈(1 + ε)· c[j2].p⌉),
19: Dj1← D− (n.d− c[j1].d)
20: return Pj1 , Dj1
5.1 Virtual proof and disproof numbers
As we set the value of node A to virtual win or loss depending on its (d)pn, we must
update other (d)pns along the path to the root. If we modify ex isting (d)pns, then other
threads can reach a state with incorrect (d)pns via transpos ition. So the use of virtual
win/loss requires the use of virtual (d)pns.
An inaccurate assignment of a virtual win or loss will cause S PDFPN search to di-
verge from DFPN search, violating condition (iii). Our initial assignments of these vir-
tual win/loss values are often accurate, as they are often made at nodes that have already
been partly searched. The amount of previous search is determined by the threshold pa-
rameter MaxWorkPerJob.
Virtual (d)pns can be stored efficiently (see below). In desc ending from the root
towards a most-proving node in order to find a state assignmen t for a thread, we use
virtual (d)pns if available, otherwise true (d)pns. After assigning a state A to a thread α ,
we update virtual (d)pns along the path to the root. Solving w ork then starts by calling
DFPN on state A. This call uses true (d)pns. When the call returns, we reset the virtual
win/loss back to a true (d)pn and update virtual (d)pns along the path to root. This
completes an iteration of a thread loop, i.e. the thread now seeks its state assignment.

<!-- page 6 -->
The entire phase of candidate-finding is guarded by a lock, so only a single thread
can operate on virtual (d)pns at a time. A lock is released onl y when DFPN is called
and solving work resumes.
Virtual (d)pns are kept in a virtual tt, which stores a node’s virtual (d)pns and the
number of threads assigned on the path to the root that contai n that node. A node is
added to the virtual tt at most as many times as the number of threads. This tt is easy to
implement for dags. Each level (move) of the game corresponds to an array whose size
is at most the number of threads. Virtual tt operations are as follows:
– VTTA DD(n): if the entry already exists, increment the counter.
– VTTR EMOVE (n): decrement the counter; if 0, remove the entry, otherwise restore
n’s previous virtual (d)pdn.
– VTTR EAD (n, j, n j): return entry for n’sjth child; if no such entry then initialize
v(d)pns by returning the default nj, which contains true (d)pns.
5.2 Finding a state candidate for a thread
Finding a candidate for state assignment can fail, as follow s. Suppose we are at node
n. We have read its (d)pns from the tt. We must select a child to descend to, so we also
read all of its children’s (d)pns from the tt. Normally, the r ecursive formulas should
give the (d)pns of n from those of its children. A child’s (d)pns can be lost due to tt
overwrites, but this is not a problem, as the (d)pns are recalculated when we descend to
such child. The problem is that c was reached via transposition and work was done at c
after the last update of n. Thus (d)pns can be stale, and an update can reveal that (d)pn
thresholds were reached or even that n has been solved.
So descending to a mpn via the usual rules is not sufficient. Instead, we recursively
search as in DFPN, using virtual (d)pns whenever they exist,and stopping the search as
soon as we find a state with whose previous work performed is below a fixed threshold
(MaxWorkPerJob). This search is performed by TRYRUNJOB, explained in §5.4.
5.3 Sharing transposition table
Threads share the tts, so we use multiple-reader/single-wr iter locks. Following [9], if
a worker thread discovers immediately before writing that t he node has (during the
worker’s processing) been solved by another thread, we do not overwrite the tt.
5.4 Implementation
Algorithm 3 shows the main scheme of SPDFPN, our parallel DFP N search. In the
loop, each thread calls T RYRUNJOB, tries to find a candidate for state assignment, and
if successful then runs a job by calling DFPN on the assigned s tate. But first we need
to update all virtual (d)pns, accessing them in nodes on the n ode-to-root path. We also
need virtual (d)pns of children of each such node. So, we intr oduce a list v directed
towards the root. Each list entry contains this data for the a ssociated node: v.n — a
node with virtual (d)pns, v.c — array of the node’s children with (d)pns, v.parent —
refer to corresponding parent’sv.

<!-- page 7 -->
Algorithm 3. Parallel DFPN
1: procedure PARALLEL DFPN( root)
2: for i = 1, . . . ,# of threads do
3: spawn thread with call R UN(root)
4: procedure RUN(n)
5: while n is not solved do
6: L OCK(job lock)
7: v.n← n, v.parent← null
8: job done← false
9: T RYRUNJOB(n, v,∞,∞)
10: U NLOCK (job lock)
11: if job done then
12: notify waiting threads
13: else
14: wait
TRYRUNJOB is shown in Algorithm 4. It works as DFPN, but additionally ca lcu-
lates virtual (d)pns and stores them in list v. Once it finds a candidate — the condition
in line 5 is true — virtual (d)pns are propagated upwards to th e root and an actual job
is run. Once this job is done the entire recursion ends, i.e. no more search is performed
and virtual (d)pns are updated by VTTR EMOVE calls.
5.5 Comparison to Kaneko’s algorithm
Kaneko parallelizes DFPN like this [9]: in an OR node’s tt acc ess, a child’s pn is in-
creased by the number of threads searching that child. Kanek o calls this augmented
value a virtual pn 3, discouraging — but not preventing — the search from repeate dly
selecting the same child. In our experiments we observe that, near tree-top, sibling pns
vary, whereas near tree-bottom, they are all small and so sim ilar. In the former case,
Kaneko’s algorithm is likely to always select the same childfor search.
By contrast, in our approach, by setting the value ofMaxWorkPerJob, we implicitly
control how deep in the tree the diversion of thread selections should occur. Moreover,
our threads always work on different subtrees.
6 Experiments
We implemented SPDFPN for Hex on the open-source Hex reposit ory Benzene [2],
which in turn is built on the open-source game-independent framework Fuego [5]. Ben-
zene uses Focussed DFPN search [1, 7], which employs an evaluation function to sort a
node’s children, and then focusses the search on a fraction of the most-promising chil-
dren. The size of the search window is given by ⌈b + f × #active children⌉, so new
children can enter the window as siblings are proved to be a lo ss. We use b = 0 and
f = 0 . 25. FDFPN search maintains the usual correctness properties o f PN search. We
used FDFPN because it is embedded in Benzene’s DFPN; our use of this DFPN variant
does not diminish the generality of SPDFPN.
Before starting our experiments, we improved Benzene’s virtual connection engine
and solver. The resulting implementation performs typical ly 2 to 10 times faster than
the previous version on similar hardware [8].
3 Do not confuse Kaneko’s augmented proof/disproof values with our definition of virtual pn.

<!-- page 8 -->
We tested SPDFPN on two sets of Hex problems: Suite 1, the thirteen previously in-
tractable 9×9 opening moves plus the (previously intractable) centremost 10×10 open-
ing move; and Suite 2, the eight hardest 8 ×8 opening moves plus eight positions from
the 2011 Olympiad Hex competition [6].
Algorithm 4. Try find a candidate and run a job
1: function TRYRUNJOB(n, v, P, D)
2: if v.n.p≥ P∨ v.n.d≥ D then
3: return 0
4: wlocal← 0
5: if n.w < MaxWorkPerJobthen
6: if n.p≤ n.d then
⊲ Virtual win
7: v.n.p← 0, v.n.d←∞
8: else
⊲ Virtual loss
9: v.n.p←∞ , v.n.d← 0
10: U PDATE VIRTUALS (v)
11: U NLOCK (job lock)
⊲ Candidate is found
⊲ Actual job is run here
12: wlocal←
13: DFPN (n, P, D, MaxWorkPerJob)
14: L OCK(job lock)
15: job done← true
16: v.n.p← n.p, v.n.d← n.d
17: VTTR EMOVE (v.n)
18: return wlocal
19: for each child j of n do
20: c[j]← TTR EAD (n, j)
21: v.c[j]← VTTR EAD (n, j, c[j])
22: loop
23: PNU PDATE(n, c)
24: PNU PDATE(v.n, v.c)
25: TTW RITE (n)
26: if job done then
27: VTTR EMOVE (v.n)
28: return wlocal
29: if v.n.p≥ P∨ v.n.d≥ D then
30: return wlocal
31: j, Pj, Dj←
32: S ELECT (v.n, v.c, P, D)
33: vchild.n← v.c[j], vchild.parent← v
34: wchild←
35: T RYRUNJOB(c[j], vchild, Pj, Dj)
36: wlocal← wlocal + wchild
37: n.w← n.w + wchild
38: procedure UPDATE VIRTUALS (v)
39: VTTA DD(v.n)
40: while v.parent is not null do
41: v← v.parent
42: PNU PDATE(v.n, v.c)
43: VTTA DD(v.n)
6.1 Previously intractable 9 × 9 and 10× 10 Hex openings
Suite 1 tests the limits of SPDFPN on 24 threads of a hyperthreaded 12-core Intel Xeon
2.93 GHz with 48 Gbyte RAM and 8 threads of an 8-core Intel Xeon 2.8 GHz with
32 Gbyte RAM. We used a tt with sizes varying from 227 to 228 entries depending on
machine and stage of a search. Here, in T RYRUNJOB, in addition to a tt we used a
database storage capable of handling simple board isomorphism (180 degree rotation).
For more difficult openings we gradually raised the value of ε from 0. 25 up to 0. 5 in
order to reduce the number of tt lookup failures.
Table 1 shows machine used and approximate running times. Du e to occasional
machine shutdown, e.g. power failure, some runs were restar ted several times from
database and tt backups; for these runs the running times arecumulative estimates based

<!-- page 9 -->
a
a
b
b
c
c
d
d
e
e
f
f
g
g
h
h
i
i
1 1
2 2
3 3
4 4
5 5
6 6
7 7
8 8
9 9
Fig. 1. Newly solved 9×9 opening values
(dots), winner if black opens there.
a
a
b
b
c
c
d
d
e
e
f
f
g
g
h
h
i
i
1 1
2 2
3 3
4 4
5 5
6 6
7 7
8 8
9 9
1
2 3
45
67
8
9
10
11
1213
14 15
16
17
18
19
20
21
2223
24
25
26
27
28
29
30
31
32
33
34
35
36
37
Fig. 2.PV ofa6, the hardest 9×9 opening.
on logs. As an indication of achieved speedup on this problem suite, the previous al-
gorithm with 8 threads failed to solve any of these openings a fter 480 hours4, whereas
SPDFPN with 24 threads solves c2 in under 33 hours. On the 9 ×9 board a6 was the
hardest opening. The behaviour of SPDFPN on this problem was different than on all
others: the main lines of play were extremely balanced, and t he winner unclear, un-
til deep into the search. See Figure 2. Although the search sp ace was around 100,000
times larger than the size of tt, SPDFPN showed continuous progress. In the previously
strongest Hex solver the search often gets stuck whenever se arch space is this much
larger than the tt [2, 1].
6.2 8 × 8 and Olympiad Hex problems
Suite 2 measures the parallel efficiency of our algorithm. On a 24-core Intel Xeon 2.4
GHz with 64 Gbyte RAM, we used 16 threads (the others were in use). We used a tt with
224 entries, which was more than sufficient. We picked moderate problems: challenging
but still tractable for a single thread. This suite consistsof eight (hardest) 8×8 openings
4 Private communication with Broderick Arneson.
a
a
b
b
c
c
d
d
e
e
f
f
g
g
h
h
i
i
j
j
1 1
2 2
3 3
4 4
5 5
6 6
7 7
8 8
9 9
10 10
1
2 3
4
5
6
7
8
9
10
11
12
13
14
15
1617
1819
20
21
22 23
24
25
26 27
28 29
30
31
Fig. 3. Principle variation off5, the first-ever solved 10×10 opening. Black wins.

<!-- page 10 -->
opening #threads time winner
a2 8/24 68d09:40:18 black
a3 8 80d08:37:34 white
a4 8 33d14:06:03 black
a5 8 65d04:14:52 black
a6 24 110d14:35:06 black
a7 24 4d08:56:03 white
a8 24 6d14:21:30 black
opening #threads time winner
b2 8 53d15:18:21 black
b4 8 29d23:53:14 black
b6 8 1d21:52:28 black
b7 8 4d17:19:13 black
c2 24 1d08:42:57 black
i1 24 6d00:51:25 black
10x10:f5 24 63d20:44:30 black
Table 1. Times (days:hrs:mins:secs) and threads for newly solved 9 ×9 and 10 ×10
openings.
and eight 11 ×11 positions from the 2011 ICGA Olympiad found by starting wi th the
final position and proceeding backwards to a moderate position. See Figures 4 and 5.
a
a
b
b
c
c
d
d
e
e
f
f
g
g
h
h
1 1
2 2
3 3
4 4
5 5
6 6
7 7
8 8
Fig. 4. The hardest 8 ×8 openings (dots),
winner if black opens there.
a
a
b
b
c
c
d
d
e
e
f
f
g
g
h
h
i
i
j
j
k
k
1 1
2 2
3 3
4 4
5 5
6 6
7 7
8 8
9 9
10 10
11 11
1
2 3
4
5
6
7
8 9
10
11
12
13
14
15
16
17
18
19
20 21
22 23
24 25
26
27
28
29
30
31
32
33
34
Fig. 5. The hardest suite 2 position, from
game 7 of the 2011 Olympiad.
6.3 Scalability
In this experiment we measured scalability, or parallel efficiency, — the average 5 speedup
ratio over serial version — for positions from suite 2. We ran our algorithm over
all instances two times. For run 1, on board size 8 ×8 (11 ×11), the default value of
MaxWorkPerJobwas 100 (20). For run 2, MaxWorkPerJobwas 500 (100). In many do-
mains, small values such as these can yield lower scalability due to thread management
overhead. However, our Hex solver spends a large fraction of the time on VC engine
computations, so for this solver these small values of MaxWorkPerJobare suitable.
Our algorithm scales well on up to 16 threads. Figure 6 shows t he scalability from
run 1 ( 9. 4 with 16 threads, or .59), and from the six hardest problems — t hree each
from 8×8 and 11×11 — from run 2 (e.g. 11. 8 with 16 threads, or .74). Table 2 shows
how time is lost due to parallelization. For each run i = 1 , 2, f i
t denotes the average
5 As usual when measuring a ratio (here, speedup), we use geometric mean for averaging.

<!-- page 11 -->
fraction of time lost due to multi-threading overhead (shar ed tt access, hardware over-
head) and f i
s denotes the average fraction of time lost due to extra leaf ex pands (extra
states searched).
run 1 run 2
n f 1
t f 1
s e1 f 2
t f 2
s e2
1 1.000 1.000 1.000 1.000 1.000 1.000
2 0.981 1.051 1.940 1.033 1.095 1.768
4 1.071 1.094 3.414 1.041 1.158 3.318
8 1.111 1.124 6.405 1.072 1.268 5.885
12 1.098 1.398 7.816 1.008 1.319 9.028
16 1.219 1.401 9.368 1.091 1.245 11.780
Table 2. Scalability.
number of threads n
scalability ei
1 2 4 8 12 16
2
4
6
8
10
12
y = x
run 1
run 2
Fig. 6. Scalability.
These values are computed as follows. Let s1 and sn be the number of leaf expands
(number of states for which the VC engine was used) for the ser ial and n-thread runs
respectively. Then f i
s = sn/s 1. Let t1 and tn be the actual running times for the serial
and n-thread runs. If there is no multi-threading overhead then t he expected time of
the n threads run is Et = t1sn/s 1/n , so f i
t = tn/E t. Thus tn = ftfst1/n , so the
scalability ei is n/ (f i
t f i
s).
7 Conclusions
We have introduced SPDFPN, a parallel version of depth-firstproof number search that
scales well. We tested our algorithm on two suites of Hex prob lems, in the process
solving all thirteen previously intractable 9×9 openings and the first-ever solution to a
10×10 opening. Our experiments showed a speedup of .74, namely 11.8 on 16 threads.
Our algorithm is general and game-independent, and so shoul d be equally effective on
any problem that can be modelled by and-or trees. It would be of interest to see whether
the SPDFPN speedups we achieved in Hex can be achieved in othe r domains, and to
compare and contrast SPDFPN to Kaneko’s parallel DFPN.
8 Acknowledgements
We thank Broderick Arneson, Yngvi Bj ¨ornsson, Phil Henderson, Aja Huang, Timo
Ewalds, Martin M¨uller, and the referees for their feedback. We thank Martin for gener-
ously loaning the use of his computing cluster for our experiments.

<!-- page 12 -->
References
1. Broderick Arneson, Ryan B. Hayward, and Philip Henderson. Solving Hex: Beyond humans.
In H. Jaap van den Herik, Hiroyuki Iida, and Aske Plaat, editors, Computers and Games
2010, volume 6515 of LNCS, pages 1–10. Springer, 2011.
2. Broderick Arneson, Philip Henderson, and Ryan B. Hayward. Ben zene, 2009-2012. http:
//benzene.sourceforge.net/.
3. D.M. Breuker, J.W.H.M. Uiterwijk, and H.J.van den Herik. Replac ement schemes and two-
level tables. ICGA, 19(3):175–180, 1996.
4. Guillaume Chaslot, Mark H.M. Winands, and H. Jaap van den Herik. Pa rallel monte-carlo
tree search. In H. Jaap van den Herik, Xinhe Xu, Zongmin Ma, and Mar k H.M. Winands,
editors, Computers and Games 2008, volume 5131 of LNCS, pages 60–71. Springer, 2008.
5. Markus Enzenberger, Martin M ¨uller, Broderick Arneson, Rich Segal, Fan Xie, and Aja
Huang. Fuego, 2007-2012. http://fuego.sourceforge.net/.
6. Ryan B. Hayward. 2011 ICGA Computer Games Olympiad Hex Competitio n Report, 2011.
http://webdocs.cs.ualberta.ca/˜hayward/papers/rptTilburg.pdf.
7. Philip Henderson. Playing and solving Hex. PhD thesis, University of Alberta, 2010.http:
//webdocs.cs.ualberta.ca/˜hayward/theses/ph.pdf.
8. Philip Henderson, Broderick Arneson, and Ryan Hayward. Solving 8×8 Hex. In Proc.
IJCAI-09, pages 505–510, 2009.
9. Tomayuki Kaneko. Parallel depth first proof number search. In Proc. AAAI-10, pages 95–
100, 2010.
10. A. Kishimoto, M.H.M. Winands, M. M ¨uller, and J-T. Saito. Game-tree search using proof
numbers: The first twenty years. ICGA, 35(3):131–156, 2012.
11. Akihiro Kishimoto. Parallel AND/OR tree search based on proof and d isproof numbers. In
5th Games Programming Workshop , volume 99 of IPSJ Symposium Series , pages 24–30,
1999.
12. Akihiro Kishimoto. Dealing with infinite loops, underestimation, and overe stimation of
depth-first proof-number search. In Proc. AAAI-10, pages 108–113, 2010.
13. Akihiro Kishimoto and Martin M ¨uller. A solution to the ghi problem for depth-first proof-
number search. Information Sciences, 175(4):296–314, 2005.
14. Akihiro Kishimoto and Martin M ¨uller. About the completeness of depth-first proof-number
search. In H. Jaap van den Herik, X. Xu, Z. Ma, and Mark H.M. Winands, editors, Computers
and Games, volume 5131 of LNCS, pages 146–156. Springer, 2008.
15. Fabien Letouzey. Fruit, 2004-2013. http://www.fruitchess.com/.
16. Ayumu Nagai. A new AND/OR tree search algorithm using proof numbe r and disproof
number. In Proceeding of Complex Games Lab Workshop, pages 40–45, Tsukuba, November
1998. ETL.
17. Ayumu Nagai. A new depth-first-search algorithm for and/or tree. Master’s thesis, University
of Tokyo, Japan, 1999.
18. Ayumu Nagai. Df–pn Algorithm for Searching AND/OR Trees and its Applications . PhD
thesis, University of Tokyo, Japan, 2002.
19. Jakub Pawlewicz and Lukasz Lew. Improving depth-first pn-sea rch: 1+ε trick. In H. ˜Jaap
van den Herik, P. Ciancarini, and H.H.L.M(J.) Donkers, editors,Computers and Games 2006,
volume 4630 of LNCS, pages 160–170. Springer, 2007.
20. Tord Romstad. Stockfish, 2008-2013. http://stockfishchess.org/.
21. Abdallah Saffidine, Nicolas Jouandeau, and Tristan Cazenave. So lving breakthrough with
race patterns and job-level proof number search. In H. Jaap van de n Herik and Aske Plaat,
editors, Advances in Computers and Games 2011 , volume 7168 of LNCS, pages 196–207.
Springer, 2012.

<!-- page 13 -->
22. Jahn-Takeshi Saito, Mark H.M. Winands, and H. Jaap van den He rik. Randomized paral-
lel proof-number search. In H. Jaap van den Herik and Pieter Spron ck, editors, Advances
in Computer Games , volume 6048 of Lecture Notes in Computer Science , pages 75–87.
Springer Berlin Heidelberg, 2010.
23. Jonathan Schaeffer, Neil Burch, Yngvi Bj¨ornsson, Akihiro Kishimoto, Martin M¨uller, Robert
Lake, Paul Lu, and Steve Sutphen. Checkers is solved. Science, 317:1518–1522, 2007.
24. I-Chen Wu, Hung-Hsuan Lin, Ping-Hung Lin, Der-Johng Sun, Yi- Chih Chan, and Bo-Ting
Chen. Job-level proof-number search for connect6. In H. Jaap van den Herik, Hiroyuki Iida,
and Aske Plaat, editors, Computers and Games 2010, volume 6515 of LNCS, pages 11–22.
Springer, 2011.
25. Kazuki Yoshizoe, Akihiro Kishimoto, and Martin M ¨uller. Lambda depth-first proof number
search and its appplication to go. In Proc. IJCAI-07, pages 2404–2409, 2007.