# Massively Parallel Proof-Number Search for Impartial Games and Beyond

**Tomáš Čížek¹, Martin Balko¹, Martin Schmid¹,²**

¹ Department of Applied Mathematics, Charles University, Prague
² EquiLibre Technologies

> Extracted from `pns-pdfpn-2025.pdf` (automated text extraction with
> pypdf; ligatures normalized; figures, tables, and display equations are
> lossy — the PDF is authoritative). Published as: Čížek, T., Balko, M.,
> Schmid, M.: Massively Parallel Proof-Number Search for Impartial Games
> and Beyond. arXiv:2511.10339v2 (9 Feb 2026); copyright AAAI-26.
> Mined in: `docs/plans/parallel/research_cizek2025.md` (two-level
> decomposition, shared worker info, 4–16 core extrapolation, option-D
> mapping).

<!-- page 1 -->

Massively Parallel Proof-Number Search for Impartial Games and Beyond
Tom´aˇs ˇC´ıˇzek1, Martin Balko1, Martin Schmid1,2
1Department of Applied Mathematics, Charles University, Prague, Czech Republic,
2EquiLibre Technologies, Inc.
{cizek, balko, schmid}@kam.mff.cuni.cz
Abstract
Proof-Number Search is a best-first search algorithm with
many successful applications, especially in game solving. As
large-scale computing clusters become increasingly accessi-
ble, parallelization is a natural way to accelerate computation.
However, existing parallel versions of Proof-Number Search
are known to scale poorly on many CPU cores. Using two par-
allelized levels and shared information among workers, we
present the first massively parallel version of Proof-Number
Search that scales efficiently even on a large number of CPUs.
We apply our solver, enhanced with Grundy numbers for re-
ducing game trees of impartial games, to the Sprouts game,
a case study motivated by the long-standing Sprouts Conjec-
ture. Our algorithm achieves 332.9×speedup on 1024 cores,
significantly improving previous parallelizations and outper-
forming the state-of-the-art Sprouts solver GLOP by four or-
ders of magnitude in runtime while generating proofs 1,000×
more complex. Despite exponential growth in game tree size,
our solver verified the Sprouts Conjecture for 42 new posi-
tions, nearly doubling the number of known outcomes.
Code— https://github.com/cizektom/spots
Introduction
Game-solving is a well-established and difficult task in arti-
ficial intelligence, which involves computing outcomes un-
der perfect play of all players, often requiring a deep traver-
sal of vast game trees. Despite these challenges, several
classical games have already been solved, including Con-
nect Four (Allis 1988), Gomoku (Allis, van den Herik, and
Huntjens 1996), Checkers (Schaeffer et al. 2007), and Oth-
ello (Takizawa 2023).Proof-Number Search(Allis, van der
Meulen, and van den Herik 1994) is among the most suc-
cessful game-solving algorithms. It is a best-first tree search
algorithm designed to compute game outcomes efficiently.
This algorithm has been widely used due to its ability to fo-
cus on the most promising parts of the game tree, making it
particularly effective in domains with large branching fac-
tors or unbalanced trees. Beyond games, its variants and re-
lated algorithms have been applied in various other settings,
for example, in chemistry (Franz et al. 2022), graphical
models (Dechter and Mateescu 2007), medicine (Heifets and
Jurisica 2012), and theorem proving (Lample et al. 2022).
Copyright © 2026, Association for the Advancement of Artificial
Intelligence (www.aaai.org). All rights reserved.
With increasing computational power, it is natural to pur-
sue parallel versions of Proof-Number Search; yet, despite
many efforts, existing variants have failed to scale efficiently
across many CPUs. Several studies have suggested using
parallel Proof-Number Search or its variants in diverse do-
mains (Franz et al. 2022; Kishimoto, Marinescu, and Botea
2015), but scalability remains a challenge. These limitations
led Kishimoto et al. (2012) to pose the problem of develop-
ing a well-scaling, massively parallel Proof-Number Search.
Many combinatorial games remain far from being solved,
including Chess, Shogi, Go, Cram, or Sprouts. InSprouts,
two players alternately connectngiven spots in the plane ac-
cording to simple rules, where the last player unable to make
a move loses; see ˇC´ıˇzek and Balko (2021) for the imple-
mentation. We use this game as our experimental domain, as
it features highly unbalanced game trees with large branch-
ing factors, making it well-suited for Proof-Number Search.
Designed by Conway and Paterson to resist computer analy-
sis (Roberts 2015), Sprouts poses a challenging benchmark:
the complexity of its game tree surpasses Chess and Go for
relatively small values ofn; see Figure 1. The computation
of outcomes of Sprouts positions is further motivated by the
long-standingSprouts Conjecture(Applegate, Jacobson, and
Sleator 1991), which states that then-spot position, consist-
ing ofngiven spots, is winning for the first player if and
only ifnis congruent to 3, 4, or 5 modulo 6.
10 20 30 40 50 60 70 80 90 100
Number of Spots
1050
10100
10150
10200
10250
10300
10350
10400
10450
Estimated Game Tree Complexity
Sprouts Tree
Sprouts Tree with GN
Checkers
Chess
Shogi
Go
Figure 1: Estimated game tree complexity of Sprouts (with
and without Grundy numbers) compared to other games.
arXiv:2511.10339v2  [cs.AI]  9 Feb 2026

---

<!-- page 2 -->

Our Contributions
We address the problem posed by Kishimoto et al. (2012)
and develop a new variant of Proof-Number Search that
scales efficiently on many CPUs. Building on this, we im-
plement a parallel solver for combinatorial games. Together
with a new memory-efficient algorithm for impartial games,
we achieve numerous new results for the game Sprouts.
Well-Scaling Parallel Proof-Number Search.We intro-
ducePNS-PDFPN, a massively parallel Proof-Number Sea-
rch for distributed-memory systems combining two paral-
lelized levels with shared information among workers. It
achieves 208.6×speedup on 1024 cores over sequential
DFPN, significantly outperforming prior methods, whose
best scaling factor in the same setting was 21.3. Using min-
imal domain knowledge, the speedup increases to 332.9×.
DFPN Search with Grundy Numbers.Proof-Number
Search is designed to solve problems that are represented as
AND/OR trees. We formalize the notion of game trees with
Grundy numbers to reduce the complexity of game trees of
the so-called impartial games, as introduced by Lemoine and
Viennot (2012). We adaptDepth-First Proof-Number Search
with Grundy numbers, a popular and memory-efficient vari-
ant of Proof-Number Search, to work with such trees.
Solving Sprouts Positions.To demonstrate the efficiency
of our new algorithms, we implemented a solver for the
Sprouts game, and used it to determine many new outcomes.
Our solver outperforms the state-of-the-art solverGLOP
by Lemoine and Viennot (2015), achieving a four-order-of-
magnitude speedup and generating proofs 1,000 times more
complex. Despite the exponential growth of the game trees,
our solver verifies the Sprouts Conjecture for 42 new posi-
tions, nearly doubling the number of known outcomes.
Related Work
A considerable amount of work has been devoted to design-
ing parallel versions of Proof-Number Search. These include
RP-PNS, introduced by Saito, Winands, and van den Herik
(2010), a parallel variant of DFPN by Kaneko (2010), and
SPDFPNalgorithm by Pawlewicz and Hayward (2014). All
of these were designed for shared-memory systems, which
typically offer only a limited number of CPU cores. To lever-
age the greater computational power of clusters with many
cores, algorithms tailored to distributed-memory systems are
required. Such algorithms includeParaPDSby Kishimoto
and Kotani (1999),Job-Level Proof-Number Searchby Wu
et al. (2011, 2013), andPPN 2 search by Saffidine, Jouan-
deau, and Cazenave (2012). A distributed-memory paral-
lel Proof-Number Search was also used by Schaeffer et al.
(2005, 2007) in solving Checkers.
As noted by Kishimoto et al. (2012), the scalability of ex-
isting algorithms was evaluated only on a relatively small
number of cores, and, despite various successes in game
solving, their efficiency declined rapidly as core counts
increased. While parallel DFPN by Kaneko (2010) and
SPDFPN have speedups of 3.58 on 8 cores and 11.8 on 16
cores, respectively, they are limited to shared-memory sys-
tems. The best scaling factors for distributed-memory sys-
tems are reported for PPN2 search, which achieved speedups
of 7.2, 11.1, and 18.3 on 16, 32, and 64 cores, respectively.
Determining the outcomes ofn-spot Sprouts positions has
a long history dating back to the 1960s. Conway himself
determined the outcomes forn≤3, and later Mollison ex-
tended the results ton= 6by hand (Gardner 1967). The first
computer-based solver was developed by Applegate, Jacob-
son, and Sleator (1991), who used it to prove outcomes for
n≤11. In 2006, Purinton created the programAuntBeast,
which solved positions up to 14 spots. A major advance was
made by the state-of-the-art solver GLOP (Lemoine and Vi-
ennot 2015) who used Alpha-Beta Pruning combined with
Grundy number theory to solve all positions forn≤32.
These results were later extended to all values ofn≤44and
selected cases up ton= 53, by incorporating a basic variant
of Proof-Number Search combined with Grundy numbers.
All known outcomes agree with the Sprouts Conjecture.
Preliminaries
We now briefly review relevant preliminaries from combi-
natorial game theory; for more details, see Kishimoto et al.
(2012) and Berlekamp, Conway, and Guy (2003). Acom-
binatorial gameis a two-player, deterministic game with
perfect information that ends in a finite number of moves.
Undernormal play convention, the last player able to move
wins. A combinatorial game isimpartialif the available
moves from any given position are the same for both players,
regardless of whose turn it is. Combinatorial games include
Chess, Checkers, Go, Othello, and Shogi, whereas impartial
games include Nim, Sprouts, Quarto, and Cram.
Game Trees in Negamax Form
Game trees of combinatorial games can be represented
by the so-calledAND/OR trees. Here, we instead work
with AND/OR trees in negamax fashion usingNAND trees,
which are easier to work with, as they eliminate the unnec-
essary distinction between the AND and OR nodes in sub-
sequent definitions. The nodes at odd and even levels of a
NAND tree correspond to positions where the first and sec-
ond player, respectively, is on the move. In particular, the
root corresponds to the initial position with the first player
on the move. Aterminalis a node with no children, which
represents a position where the game ends. In a partially ex-
panded NAND tree, a non-terminal node isinternalif some
of its children are generated, and aleafif none of them are.
Each node is associated with one of the followingvalues:
win, loss, or unknown. The value of a node isunknownif
the outcome of the associated position is not implied by the
currently expanded subtree of the node. Otherwise, it iswin
orloss, depending on whether the player on the move at the
associated position has a winning strategy or not. Thus, the
value of every terminal is loss in any combinatorial game
under the normal play convention. The value of an internal
node is win if at least one of its children is loss, and it is loss
if all its children are wins; otherwise, the value is unknown.
The value of a leaf is always unknown. A node isprovedor
disprovedif its value equals to win or loss, respectively.
Given a positionP, the goal is to find aproofofP, which
is a NAND tree rooted inPwhose value is known. These

---

<!-- page 3 -->

proofs correspond to the so-calledweak solutions, as they
give the outcome ofPand a winning strategy for one of the
players. In contrast,strong solutionsprovide outcomes and
winning strategies for all positions reachable in the game.
Proof-Number Search
We first describe a basic variantPNSof the Proof-Number
Search for NAND trees. In each step, PNS selects and ex-
pands amost proving node(MPN)—a leaf in a currently ex-
panded tree whose solution would contribute the most to the
solution of the root. To find an MPN, each nodevin a NAND
tree is associated with aproof numberpn(v)∈N 0∪{∞}
and adisproof numberdn(v)∈N 0∪{∞}, representing the
lower bounds on the minimum number of leaves in the sub-
tree ofvthat have to be solved to prove or disprovev, respec-
tively. Both these values are computed in a bottom-up man-
ner. For leaves, they are initialized aspn(v) =dn(v) = 1.
For terminals in games under the normal play convention,
pn(v) =∞anddn(v) = 0. The valuespn(v)anddn(v)of
an internal nodevare defined aspn(v) = min c dn(c)and
dn(v) = P
c pn(c), where the minimum and the sum are
taken over the childrencofv. At the start, the algorithm ini-
tializespn(r)anddn(r)of the rootr. At each step, it then
finds an MPN by descending from the root and always se-
lecting the child with the lowest disproof number with ties
broken arbitrarily. PNS then expands the selected MPN by
generating all its childrencand initializing theirpn(c)and
dn(c). At the end of each step, it updates the proof and dis-
proof numbers on the path back to the root.
Depth-First Proof-Number Search.One drawback of
PNS is that it stores the entire expanded tree in mem-
ory, which is usually consumed very quickly. Therefore,
Nagai (2002) introducedDepth-first Proof-Number Search
(DFPN), a space-efficient variant of PNS.
DFPN selects the MPN in a depth-first search manner. Let
Pbe a currently explored path consisting of internal nodes
leading to the MPN. The key idea of DFPN allows one to
store only the nodes alongPtogether with their children
while preserving the properties of PNS and staying deep in
the tree as long as the MPN is guaranteed to occur there.
To achieve that, for each nodevofP, DFPN maintains two
thresholdspt(v)∈N 0∪{∞}anddt(v)∈N 0∪{∞}for its
proof and disproof numbers. The MPN occurs in the subtree
ofvif and only ifpn(v)< pt(v)anddn(v)< dt(v), where
∞<∞is interpreted as false. Otherwise, DFPN backtracks
alongPuntil it reachesv ′ that satisfiespn(v′)< pt(v′)and
dn(v′)< dt(v ′), updating the proof and disproof numbers
alongP. DFPN then resumes MPN selection fromv ′.
First, setpt(r) =dt(r) =∞for the rootr. Letvbe
the currently visited node,wandw ′ be its children with the
lowest and second lowest disproof number, wherewis the
next node to be selected. The thresholds ofware then set as
pt(w) =dt(v)−dn(v) +pn(w),
dt(w) = min{pt(v), dn(w′) + 1}. (1)
The decreased memory consumption of DFPN comes at the
cost of increased time complexity since the proof and dis-
proof numbers of some of the previously visited nodes are
lost and potentially need to be recomputed. To balance this
trade-off, DFPN is often combined with a transposition table
that maintains previously computed values.
Grundy Numbers
As shown by Lemoine and Viennot (2012) and by Bel-
ing and Rogalski (2020), theSprague–Grundy Theorem
(Grundy 1939; Sprague 1935) can be applied to effectively
simplify game trees of impartial games under the normal
play convention; see Figure 1. Since we build on these tech-
niques, we briefly recall the necessary background.
Nimis a game that is played onhheaps withn 1, . . . , nh∈
N0 objects. With each move, a player must remove at least
one object from a single heap, and the first player with no
move loses the game. Nim is a strongly solved game, as Bou-
ton (1901) proved that the outcome of Nim is loss if and only
ifn 1⊕···⊕n h = 0, where⊕is a bitwise exclusive or.
LetP 1, . . . , Pk be positions of an impartial game un-
der the normal play convention. Then, thecombinationof
P1, . . . , Pk, denoted byP 1 +···+P k, is a position in
which, in each turn, the players decide to move in one of the
positionsP 1, . . . , Pk while leaving the other positions un-
touched. The first player with no move in any ofP 1, . . . , Pk
loses in the combinationP 1 +···+P k. A positionPis
atomicif it cannot be expressed as a combination of at least
two non-empty positions; otherwise, it isdecomposable, in
which caseP=P 1 +···+P k for non-empty atomic po-
sitionsP 1, . . . , Pk andk≥2. Two positionsPandQare
equivalentif, for any positionR, the combinationsP+R
andQ+Rhave the same outcome.
TheGrundy numbergn(P)of a positionP(also called
thenimberofP) is defined recursively as follows. IfPis
a terminal position, thengn(P) = 0. Otherwise,gn(P)
is equal tominN 0\GwhereGis the set of the Grundy
numbers of the children ofP. Using∗nto denote a Nim
position with a single heap ofnobjects, we formulate the
Sprague–Grundy Theorem(Grundy 1939; Sprague 1935),
which states that each equivalence class of impartial games
under the normal play convention can be represented by a
unique Grundy number.
Theorem 1(The Sprague–Grundy Theorem).Each position
Pof an impartial game under the normal play convention is
equivalent to∗gn(P).
It follows from Theorem 1 that the outcome ofP1 +···+
Pk is loss if and only ifgn(P1)⊕···⊕gn(P k) = 0. Thus, we
can compute the outcome of a decomposable positionP=
P1 +···+P k more efficiently using the Grundy numbers
gn(P1), . . . , gn(Pk)(Lemoine and Viennot 2012).
Methods
First, we formalize extended NAND trees with Grundy num-
bers that were implicitly used by Lemoine and Viennot
(2012) to efficiently determine the outcomes of decompos-
able positions. Then, we describe and improve PNS for ex-
tended NAND trees by Lemoine (2011) and introduce DFPN
for such trees. Finally, we describe our massively parallel
version PNS-PDFPN of Proof Number Search.

---

<!-- page 4 -->

P1 + · · · + Pk + ∗n
P1
Pk−1
Pk + ∗n′
P1 + ∗0
P1 + ∗c(P1)
Pk + ∗0
Pk + ∗(n′ − 1)
C1 + ∗n′
Cm + ∗n′
· · ·
· · ·
· · ·
· · ·
· · ·
d
g
a
a
a
a
a
g
Figure 2: A scheme of a NAND tree with Grundy numbers.
We useC 1, . . . , Cm for the children ofP k and d , a , g
for decomposable, atomic, and Grundy nodes, respectively,
and for nodes that can be atomic or decomposable.
NAND Trees with Grundy Numbers
To apply the Sprague–Grundy Theorem, we need to deter-
mine the Grundy numbergn(P)of a positionPin an im-
partial gameGunder the normal play convention. We effi-
ciently computegn(P)using the following recursive proce-
dure introduced by Lemoine and Viennot (2012): start with
n= 0and incrementnuntil the outcome of the combination
P+∗n, called acouple, is loss. Thengn(P) =n, because
gn(P) =nif and only if the outcome ofP+∗nis loss.
IfPis atomic, then the outcome of the coupleP+∗nis
obtained as before from the outcomes of its children, which
are the couples of the formP′+∗nandP+∗n ′ whereP′ is a
child ofPand0≤n ′ < n. The outcome ofP+∗nwith de-
composableP=P 1+···+P k is obtained by first computing
gn(P1), . . . , gn(Pk−1)using the above procedure and then
by computing the outcome of the coupleP k +∗n′, where
n′ =n⊕gn(P 1)⊕···⊕gn(P k−1)andP k is now atomic.
The nodes of a NAND tree with Grundy numbers cor-
respond either to atomic positionsQofGor to couples
P+∗nconsisting of a positionPofGand a Nim position∗n
withn∈N 0. We have three types of nodes: decomposable,
atomic, and Grundy; see Figure 2. A nodevcorresponding
to a coupleP+∗nisdecomposableifPis decomposable
andatomicifPis atomic. Each nodeucorresponding to an
atomicQis aGrundynode. The children ofvcorrespond to
the children ofP+∗nifPis atomic and to Grundy nodes
P1, . . . , Pk−1 and the coupleP k +∗n′ ifPis decomposable.
The children ofucorrespond toQ+∗0, . . . , Q+∗c(Q),
wherec(Q)is the number of children ofQ.
The value of a nodevcorresponding toP+∗nis again ei-
ther win, loss, or unknown. For terminals, which correspond
to positions∅+∗0, where∅is the empty position inG, the
value is loss. Ifvis atomic, then its value is determined in
the same way as in the original NAND tree with respect to
the children ofP+∗n. Ifvis decomposable, then the value
is unknown ifvis a leaf, and is equal to the value of the node
corresponding toP k +∗n′ ifvis internal. For a Grundy node
ucorresponding toQ, the value ofuis either a Grundy num-
bergn(Q)or unknown. The value ofuequalsnif there is a
losing childQ+∗nofQ, and is unknown otherwise.
Proof-Number Search with Grundy Numbers
To extend PNS to NAND trees with Grundy numbers, we
only need to adapt the proof and disproof numbers and spec-
ify the next nodewto be selected. The resulting algorithm
PNS with GNthen proceeds exactly as PNS.
We define the proof and disproof numbers so that they
give lower bounds on the size of the proof of a given po-
sition, which guarantees that PNS finds a minimal proof.
For leaves, we initialize them to1. For atomic nodes, the
proof and disproof numbers and the next nodeware de-
fined as in the original setting. Ifuis a Grundy node, then it
corresponds to an atomic positionQand we usev i to de-
note the child ofurepresentingQ+∗i. The children of
uare atomic and are generated incrementally with increas-
ingi, each only after the previous one has been proved,
as prescribed by the recursive procedure for the calcula-
tion ofgn(Q). Ifv j is the last child ofugenerated so far,
then we setpn(u) =dn(u) = min{pn(v j), dn(vj)}and
w=v j. Finally, letvbe a decomposable node. Thenvcor-
responds to a coupleP+∗nwhereP=P 1 +···+P k
andk≥2. We useu 1, . . . , uk to denote the nodes where
eachu i is a Grundy node corresponding toP i andv ′
k is
an atomic node corresponding toP k +∗n′. Note that only
u1, . . . , uk−1, v′
k are the children ofv, and that in order to
generatev′
k, we first need to know the Grundy numbers of
P1, . . . , Pk−1, asn′ =n⊕gn(P 1)⊕···⊕gn(P k−1). Thus,
ifv ′
k has not been generated, we useu k instead and set
pn(v) =dn(v) = Pk
i=1 pn(ui)andwto be the first node
fromu 1, . . . , uk−1 with unknown Grundy number. Other-
wise, we letpn(v) =pn(v ′
k),dn(v) =dn(v ′
k), andw=v ′
k.
We improve this version of PNS from Lemoine (2011) by
following Schiff, Allis, and Uiterwijk (1994) and merging
nodes representing identical states to transform the tree into
a directed acyclic graph. This significantly reduces the com-
putation time without increasing memory overhead when
combined with storing all touched nodes.
DFPN with Grundy Numbers.We now introduce a new
variant of DFPN for extended NAND trees with Grundy
numbers (DFPN with GN). The algorithm follows the same
structure as DFPN for NAND trees, requiring only modifi-
cations to the thresholds and the way they are computed.
For each nodevfrom the currently explored pathP, we
again maintain thresholdspt(v)anddt(v)upper-bounding
proof and disproof numbers. However, this time we also
need to maintain a new thresholdmt(v)∈N 0∪{∞}with
parametersps(v)∈N 0 andds(v)∈N 0. Intuitively,mt(v)
bounds the minimum ofpn(v)anddn(v), shifted byps(v)
andds(v), which is necessary due to decomposable nodes.
To simplify the definition of the thresholds, we replace
each Grundy nodeuin its decomposable parent with the
nodev j, which would be selected as the next node if we
were inu. Then, we can keep the thresholds only for atomic
and decomposable nodes. Now, letvbe the currently vis-
ited node, and letwbe the next node to be selected. For an
atomicv, we letw ′ be its child with the second-lowest dis-
proof number, and we set the thresholds as
pt(w) =dt(v)−dn(v) +pn(w),
dt(w) = min{pt(v), dn(w′) + 1},
mt(w) =mt(v),
(2)

---

<!-- page 5 -->

whereps(w) =ds(v)+dn(v)−pn(w)andds(w) =ps(v).
For a decomposablev, we setpt(w) =pt(v),dt(w) =
dt(v), andmt(w) =mt(v)ifv ′
k has been generated. Ifv ′
k
has not been generated yet, we define the thresholds as
pt(w) =dt(w) =∞,
mt(w) =t(v)−pn(v) + min{pn(w), dn(w)},
whereps(w) =ds(w) = 0andt(v) = min{pt(v), dt(v),
mt(v)−min{ps(v), ds(v)}}. For the rootr, we initialize
pt(r) =dt(r) =mt(r) =∞andps(r) =ds(r) = 0. With
this choice of thresholds, we obtain the following result.
Theorem 2.For every nodevofP, the MPN is in the sub-
tree ofvif and only ifpn(v)< pt(v),dn(v)< dt(v), and
min{pn(v) +ps(v), dn(v) +ds(v)}< mt(v).
See Appendix for the proof. To balance the time com-
plexity and memory consumption of DFPN with GN, we
also incorporate a transposition table of proof and disproof
numbers, maintained by the replacement strategy proposed
by Nagai (2002). In addition, we store the Grundy numbers
derived during computation in a separate database. Since
this database is much smaller and fits easily into memory,
we do not apply any replacement strategy to it.
PNS-PDFPN Algorithm
Our new parallel variantPNS-PDFPNof Proof-Number
Search operates on two parallelized levels. The first PNS
level targets distributed-memory systems and is based on
Job-Level Proof-Number Search used by Schaeffer et al.
(2007), Saffidine, Jouandeau, and Cazenave (2012), and Wu
et al. (2013). At this level, themasterprocess maintains
the current proof state and repeatedly assigns jobs towork-
ersfor asynchronous second-level processing. At the sec-
ond level, each worker performs the parallel DFPN algo-
rithm by Kaneko (2010), which we callPDFPN, to utilize
shared memory within a single cluster node. We also intro-
duce the sharing of key results between workers to reduce
search overhead. We now describe both levels in detail.
First-Level Parallelization.The master and workers typ-
ically run on separate nodes of a cluster and communicate
over an interconnected network. When a worker becomes
idle, the master selects a so-called pseudo-MPN leafℓand
assigns it to the worker. The worker then processesℓasyn-
chronously until it is solved or the maximum number of ex-
pansions is reached. The resulting proof and disproof num-
bers ofℓand its children are then sent back to the master,
which uses them to expandℓin the master tree and updates
its proof and disproof numbers. During processing ofℓ, the
worker periodically sends updated valuespn(ℓ)anddn(ℓ)to
keep the master tree as up-to-date as possible. To prevent re-
assignment of the same jobs, the assigned leaves are locked,
and the definition of proof and disproof numbers is slightly
adjusted to avoid misleading attraction to the locked leaves;
see Wu et al. (2013) for details.
Similar to Liang, Wei, and Wu (2015) and their JL-UCT
Search, our master maintains a database of key computed
results shared with all workers to reduce search overhead.
Newly derived results are sent to workers along with their
assigned jobs, while workers report new results back to the
master with each update. Finally, if multiple workers are lo-
cated on the same node, then they are grouped together to
share the same local version of the database.
Second-Level Parallelization.As cluster nodes consist of
multiple CPU cores, it becomes advantageous, especially
with an increasing number of workers, to start paralleliz-
ing the workers themselves, rather than adding more workers
that would otherwise process less relevant jobs. This worker
reduction also decreases communication overhead and al-
lows sharing of proof and disproof numbers within a node.
Our workers are based on DFPN to make full use of avail-
able memory; thus, we parallelize them using PDFPN. In
this algorithm, there are as many threads as the number
of cores assigned to the worker. Each thread performs an
independent DFPN search in the subtree ofℓ, while shar-
ing the lock-protected transposition table of proof and dis-
proof numbers. To decrease redundant thread computation in
shared subtrees, the disproof numbers are virtually increased
during the selection of the next nodewby the number of
threads currently computing in the corresponding subtrees.
The thresholds in (1) and (2) are then modified by subtract-
ingth(w)from the termdn(w ′) + 1, whereth(w)is the
number of threads in the subtree ofw. Additionally, if a
thread solves a position currently computed by a different
one, the other thread is notified to backtrack to that position.
Enhancements for Impartial Games.To employ the the-
ory of Grundy numbers, we incorporate PNS with GN and
DFPN with GN. We use all computed Grundy numbers as
the key shared results, since they are highly reusable and in-
expensive to share. To enable parallel computation of mul-
tiple children of a decomposable node, the next nodew
within a decomposable node is selected so that the value
min{pn(w), dn(w)}is minimum, rather than always select-
ing the first unsolved child.
Experiments
We implementedSPOTS, a PNS-PDFPN-based solver for
combinatorial games, augmented with Grundy numbers to
reduce game trees of impartial games effectively. From now
on, we use PNS and DFPN to refer to their Grundy-enhanced
variants. We evaluate the performance of SPOTS on Sprouts,
an impartial game well-suited for Proof-Number Search due
to its highly unbalanced game trees, large branching factors,
and the fact that it often naturally decomposes into indepen-
dent subpositions. To incorporate Sprouts into SPOTS, we
implemented the string-based position representation from
Applegate, Jacobson, and Sleator (1991). Our experiments
were carried out on the 29-spot and 47-spot positions, which
are the largest that allow each trial to complete within 24
hours. The experimental results are reported as the mean±
standard error of the mean, based on three independent mea-
surements.
Efficiency of DFPN with GN
In Figure 3, we demonstrate how DFPN with GN allows for
tuning the trade-off between computation time and memory

---

<!-- page 6 -->

103 104 105 106 107
Nodes Stored
102
103
104
105
Computation Time [s]
SPOTS (DFPN) SPOTS (PNS) GLOP (PNS)
DFPN(103)
DFPN(104)
DFPN(105)
DFPN(106)
S-PNS
G-PNS
Figure 3: Performance of PNS implementations in GLOP
and SPOTS and of DFPN(C) in SPOTS on the 29-spot po-
sition, whereCis the capacity of the transposition table.
consumption. By adjusting the capacityCof the transpo-
sition table, memory requirements can be substantially re-
duced at the cost of a moderate increase in computation time.
We observe that the transposition table withC= 10 6 is al-
ready sufficiently large, as it is not fully saturated. Despite
this, our PNS implementation still outperforms this DFPN
configuration, as it operates faster on its explicitly expanded
tree; however, the PNS tree can grow indefinitely.
We also compare our PNS with the PNS implemented by
Lemoine and Viennot (2015) in their solver GLOP. Unlike
our method, GLOP operates on a proper tree without trans-
positions and discards all visited nodes in the solved sub-
trees to reduce memory usage. Along with a roughly three-
times faster state representation, our design choices lead to a
roughly 100-fold reduction in computation time compared to
GLOP while maintaining a comparable maximum tree size.
Scaling Efficiency of PNS-PDFPN
PNS-PDFPN includes five tunable parameters:workers,
the total number of workers;iterations, the maxi-
mum number of expansions per job;updates, the num-
ber of iterations between each update sent to the master;
grouping, the number of workers grouped within a sin-
gle node; andthreads, the number of threads assigned to
PDFPN per worker. The number of CPU cores used by PNS-
PDFPN is then equal toworkerstimesthreadsplus the
number of cores allocated to the master. We now analyze the
effect of the worker synchronization and parameter setting
on the scaling efficiency of the algorithm, which is reported
with respect to the number of CPU cores allocated to work-
ers, excluding the master CPU.
Retaining and Sharing Second-Level Information.The
parallelPPN 2 search by Saffidine, Jouandeau, and Cazenave
(2012) corresponds to PNS-PNS using PNS at both lev-
els. However, after each job is completed, the second-level
PNS search tree is discarded, losing the entire worker state.
In contrast, our PNS-DFPN workers retain their transpo-
1 2 4 8 16 32 64 128 256 512
CPU Cores
25
26
27
28
29
210
211
212
213
Computation Time [s]
DFPN(106)
PNS-PNS
PNS-PNS + GN
PNS-DFPN + GN
PNS-DFPN + GN Sync
Figure 4: Comparison of PNS-PNS and PNS-DFPN, includ-
ing the impact of retention of Grundy numbers (GN) in
workers and their synchronization (GN Sync). All variants
are run on the 29-spot position with 100iterations, 100
updates, no grouping, and no second-level parallelization.
sition tables, completely preserving their state throughout
the whole computation. Figure 4 shows how the retention
and sharing of the worker state impact scaling efficiency.
Although PNS-PNS scales well to up to 64 cores, worker
state resets hamper its performance so significantly that it is
outperformed by sequential DFPN(106). However, when the
key results are retained, specifically the computed Grundy
numbers, the speedup improves significantly. Further per-
formance gains can be achieved by PNS-DFPN preserving
computed proof and disproof numbers. However, this has a
smaller effect than retaining the Grundy numbers. Finally,
sharing Grundy numbers among workers yields a further
significant performance gain.
PNS-PDFPN Ablations.We evaluate the impact of the
PNS-PDFPN parameters on the scaling efficiency, measured
relative to the runtime of DFPN(5·10 7), serving as a base-
line for comparing various parallel Proof-Number Search
algorithms. We compare PNS-PDFPN against PDFPN by
Kaneko (2010) and the PNS-DFPN scheme employed by
Schaeffer et al. (2007) to solve Checkers, which, according
to our measurements, represent state-of-the-art approaches
for shared- and distributed-memory systems, respectively.
In Figure 5, we incrementally add parameters until we
arrive at the full PNS-PDFPN with all the improvements.
For each setup, we report the speedup achieved using the
best-performing parameter setting, where PNS-DFPN is al-
ready optimized foriterationsandupdates, as pro-
posed by Saffidine, Jouandeau, and Cazenave (2012). With-
out GN synchronization, PNS-DFPN scales very poorly. En-
abling synchronization improves scalability up to 512 cores,
but then the performance degrades due to significant syn-
chronization overhead. This issue is overcome by grouping
workers, reducing the amount of shared results, and thus the

---

<!-- page 7 -->

1 2 4 8 16 32 64 128 256 5121024
CPU Cores
0
100
200
300Speedup Relative to DFPN(5 · 107)
 PDFPN
PNS-DFPN
+ GN Sync
+ GN Sync + Gr
+ GN Sync + Gr + Th
(our PNS-PDFPN)
PNS-PDFPN + Hr
Figure 5: Impact of GN synchronization (GN Sync), group-
ing (Gr), second-level parallelization (Th), and the child-
ordering heuristic (Hr) on the final speedup of PNS-PDFPN
relative to DFPN(5·10 7), which finished in293±41.7min-
utes. Measured on the 47-spot position.
synchronization overhead. Further scalability is achieved by
introducing second-level parallelization, which ultimately
leads to a speedup of208.67±9.17without relying on any
domain knowledge, which exceeds even the best speedup of
21.35±2.40achieved by PDFPN using shared memory.
We observe a notable improvement in scaling efficiency
after incorporating the child-ordering heuristic proposed by
Lemoine and Viennot (2015) to break ties during the selec-
tion of the next nodew. Adding the heuristic does not en-
hance the overall performance if applied to optimally tuned
parameters without the heuristic, as shown in the upper part
of Table 1. However, the heuristic becomes beneficial when
the parameters are tuned specifically for its use; see the
bottom part of Table 1. The heuristic then enables more
effective utilization of second-level parallelism in longer-
running jobs by providing better guidance to branches that
are more likely to yield shorter proofs. With this modest do-
main knowledge, the final speedup of PNS-PDFPN reaches
332.97±26.8. Further parameter analysis and hardware
specification are given in Appendix.
Solving Sprouts Positions
With SPOTS, we determined 32 new outcomes ofn-spot
positions using the sequential version and 10 more with its
parallel counterpart, nearly doubling the number of previ-
ously known results; see Appendix. We verified the new
proofs using the verification software by Lemoine and Vien-
not (2015). All computed outcomes agree with the Sprouts
Conjecture, which thus remains open. Compared to the pre-
vious state-of-the-art solver GLOP, our parallel version of
SPOTS achieves a speedup of four orders of magnitude on
1024 cores. We base this estimate on the combined improve-
ments of the sequential SPOTS over GLOP and the par-
allel SPOTS over the sequential version. To illustrate the
Cores Parameters Speedup
It Th PNS-PDFPN + Heuristic
Best Values for No-Heuristic
1 1k 11.08±0.100.99±0.00
2 1k 1 1.87±0.022.36±0.28
4 1k 1 3.91±0.314.34±0.32
8 1k 17.41±0.217.10±0.53
16 10k 114.37±0.8610.15±4.48
32 10k 1 21.75±0.6823.26±0.05
64 1k 4 36.97±0.2937.93±0.21
128 1k 857.43±0.0545.76±2.32
256 1k 8 86.87±1.4292.35±1.57
512 1k 8132.44±6.11123.72±3.97
1024 1k 16208.67±9.17169.24±2.86
Best Values for Heuristic
1 1k 11.08±0.100.99±0.00
2 1k 1 1.87±0.022.36±0.28
4 1k 1 3.91±0.314.34±0.32
8 10k 1 7.36±0.418.06±0.09
16 100k 1 11.90±0.9416.22±0.53
32 100k 4 18.50±0.7626.13±3.87
64 100k 8 27.95±2.1940.86±1.13
128 100k 4 29.44±0.5387.02±0.81
256 100k 8 54.81±3.71115.11±8.88
512 100k 16 73.21±6.36203.05±9.82
1024 100k 32 97.60±9.24332.97±26.8
Table 1: Best-performing values ofiterations(It) and
threads(Th) for PNS-PDFPN without (top) and with
(bottom) the heuristic.Groupingset to its maximum and
updatesto 1,000 are optimal across all experiments.
speedup, SPOTS solves the39-spot position, one of the
most complex solved by GLOP, in just 7.7 minutes using
1024 cores. The outcomes determined by parallel SPOTS
are much more complex than those previously computed.
The most difficult position was solved in 24 days on 512
cores, and its proof size (the number of stored Grundy num-
bers) exceeds prior ones by a factor of 1,000.
Conclusion
While our experiments focus on the impartial game Sprouts,
PNS-PDFPN is domain-independent and applicable to any
combinatorial game. For non-impartial games, the solver
naturally reduces to exploring standard NAND trees, where
computed Grundy numbers correspond to loss outcomes.
Position decomposition is the only impartial-specific com-
ponent, used as an optional acceleration in all baselines,
and therefore does not affect overall scalability. Scalabil-
ity arises from multi-level parallelization and intermediate-
result sharing, both of which are general. Sprouts offers one
advantage for scaling: its relatively slow expansion rate al-
lows frequent synchronization. However, this does not favor
our algorithm, which strongly outperforms state-of-the-art
methods in the same setting. In faster-expanding domains,
we suggest using costlier search-guiding heuristics in ex-
change for improved scalability. Lastly, since even minimal
domain knowledge for job assignment notably boosts scal-
ing, more sophisticated use of domain knowledge, such as
RL-based policies, may yield further speedups.

---

<!-- page 8 -->

Acknowledgments
All authors were supported by the grant no. 25-18031S of
the Czech Science Foundation (GA ˇCR). T. ˇC´ıˇzek was sup-
ported by the Charles University Grant Agency (GAUK)
project no. 326525. M. Schmid was also supported by the
Charles University project UNCE 24/SCI/008. The authors
thank EquiLibre Technologies, Inc. for providing computa-
tional resources. Additional computational resources were
provided by the e-INFRA CZ project (ID:90254), supported
by the Ministry of Education, Youth and Sports of the Czech
Republic. Special thanks to Neil Burch for valuable com-
ments and insights.
References
Allis, L.; van der Meulen, M.; and van den Herik, H. 1994.
Proof-number search.Artificial Intelligence, 66(1): 91–124.
Allis, L. V . 1988.A Knowledge-Based Approach of Connect
Four: The Game is Over. M.Sc. thesis, Vrije Universiteit
Amsterdam.
Allis, L. V .; van den Herik, H. J.; and Huntjens, M. P. H.
1996. Go-Moku Solved by New Search Techniques.Com-
putational Intelligence, 12(1): 7–23.
Applegate, D.; Jacobson, G.; and Sleator, D. 1991. Com-
puter Analysis of Sprouts. Technical report, Carnegie Mel-
lon University.
Beling, P.; and Rogalski, M. 2020. On pruning search trees
of impartial games.Artificial Intelligence, 283: 103262, 16.
Berlekamp, E. R.; Conway, J. H.; and Guy, R. K. 2003.Win-
ning Ways for Your Mathematical Plays. Vol. 3. A K Peters.
Bouton, C. L. 1901. Nim, A Game with a Complete Mathe-
matical Theory.Ann. of Math. (2), 3(1-4): 35–39.
ˇC´ıˇzek, T.; and Balko, M. 2021. Implementation of Sprouts:
A Graph Drawing Game. In Purchase, H. C.; and Rutter, I.,
eds.,Graph Drawing and Network Visualization, 391–405.
Cham: Springer International Publishing.
Dechter, R.; and Mateescu, R. 2007. AND/OR search spaces
for graphical models.Artificial Intelligence, 171(2): 73–106.
Franz, C.; Mogk, G.; Mrziglod, T.; and Schewior, K. 2022.
Completeness and Diversity in Depth-First Proof-Number
Search with Applications to Retrosynthesis. InProceedings
of the 31st International Joint Conference on Artificial Intel-
ligence (IJCAI-22), 4747–4753. International Joint Confer-
ences on Artificial Intelligence Organization.
Gardner, M. 1967. Mathematical Games: of Sprouts and
Brussels Sprouts; Games with a Topological Flavour.Sci.
Amer., 217: 112–115.
Grundy, P. M. 1939. Mathematics and Games.Eureka, 2:
6–8.
Heifets, A.; and Jurisica, I. 2012. Construction of New
Medicines via Game Proof Search. InProceedings of the
AAAI Conference on Artificial Intelligence, AAAI Confer-
ence on Artificial Intelligence, 1564–1570.
Kaneko, T. 2010. Parallel Depth First Proof Number Search.
InProceedings of the AAAI Conference on Artificial Intelli-
gence, AAAI Conference on Artificial Intelligence, 95–100.
Kishimoto, A.; and Kotani, Y . 1999. Parallel AND/OR Tree
Search Based on Proof and Disproof Numbers. InProceed-
ings of the 5th Game Programming Workshop, volume 99,
24–30.
Kishimoto, A.; Marinescu, R.; and Botea, A. 2015. Parallel
Recursive Best-First AND/OR Search for Exact MAP Infer-
ence in Graphical Models. In Cortes, C.; Lawrence, N.; Lee,
D.; Sugiyama, M.; and Garnett, R., eds.,Advances in Neu-
ral Information Processing Systems (NIPS-15), volume 28.
Curran Associates, Inc.
Kishimoto, A.; Winands, M. H. M.; M ¨uller, M.; and Saito,
J. T. 2012. Game-Tree Search Using Proof Numbers: The
First Twenty Years.ICGA Journal, 35(3): 131–156.
Lample, G.; Lacroix, T.; Lachaux, M.; Rodriguez, A.; Hayat,
A.; Lavril, T.; Ebner, G.; and Martinet, X. 2022. HyperTree
Proof Search for Neural Theorem Proving. In Koyejo, S.;
Mohamed, S.; Agarwal, A.; Belgrave, D.; Cho, K.; and Oh,
A., eds.,Advances in Neural Information Processing Sys-
tems, volume 35, 26337–26349. Curran Associates, Inc.
Lemoine, J. 2011.M ´ethodes Algorithmiques pour la R ´eso-
lution des Jeux Combinatoires. Ph.D. thesis, Universit ´e des
Sciences et Technologie de Lille - Lille I. In French.
Lemoine, J.; and Viennot, S. 2012. Nimbers are inevitable.
Theoret. Comput. Sci., 462: 70–79.
Lemoine, J.; and Viennot, S. 2015. Computer Analysis of
Sprouts with Nimbers. InGames of no chance 4, volume 63
ofMath. Sci. Res. Inst. Publ., 161–181. Cambridge Univ.
Press, New York.
Liang, X.; Wei, T.; and Wu, I.-C. 2015. Job-level UCT
search for solving Hex. In2015 IEEE Conference on Com-
putational Intelligence and Games (CIG).
Nagai, A. 2002.Df-pn Algorithm for Searching AND/OR
trees and its Applications. Ph.D. thesis, The University of
Tokyo.
Pawlewicz, J.; and Hayward, R. B. 2014. Scalable Parallel
DFPN Search. In van den Herik, H. J.; Iida, H.; and Plaat,
A., eds.,Computers and Games, 138–150. Cham: Springer
International Publishing.
Roberts, S. 2015.Genius at Play: The Curious Mind of John
Horton Conway. New York: Bloomsbury Press.
Saffidine, A.; Jouandeau, N.; and Cazenave, T. 2012. Solv-
ing Breakthrough with Race Patterns and Job-Level Proof
Number Search. In van den Herik, H. J.; and Plaat, A.,
eds.,Advances in Computer Games, 196–207. Berlin, Hei-
delberg: Springer Berlin Heidelberg.
Saito, J.-T.; Winands, M. H. M.; and van den Herik, H. J.
2010. Randomized Parallel Proof-Number Search. In
van den Herik, H. J.; and Spronck, P., eds.,Advances
in Computer Games, 75–87. Berlin, Heidelberg: Springer
Berlin Heidelberg.
Schaeffer, J.; Bjoernsson, Y .; Burch, N.; Kishimoto, A.;
Mueller, M.; Lake, R.; Lu, P.; and Sutphen, S. 2005. Solving
Checkers. In Kaelbling, L. P.; and Saffotti, A., eds.,Proceed-
ings of the 19th International Joint Conference on Artificial
Intelligence (IJCAI-05), 292–297. International Joint Con-
ferences on Artificial Intelligence Organization.

---

<!-- page 9 -->

Schaeffer, J.; Burch, N.; Bj ¨ornsson, Y .; Kishimoto, A.;
M¨uller, M.; Lake, R.; Lu, P.; and Sutphen, S. 2007. Checkers
is Solved.Science, 317(5844): 1518–1522.
Schiff, M.; Allis, L. B.; and Uiterwijk, J. W. H. M. 1994.
Proof-Number Search and Transpositions.ICCA Journal,
17(2): 63–74.
Sprague, R. 1935. ¨Uber Mathematische Kampfspiele.To-
hoku Math. J., First Series, 41: 438–444.
Takizawa, H. 2023. Othello is Solved. arXiv:2310.19387.
Wu, I.-C.; Lin, H.-H.; Lin, P.-H.; Sun, D.-J.; Chan, Y .-C.;
and Chen, B.-T. 2011. Job-Level Proof-Number Search for
Connect6. In van den Herik, H. J.; Iida, H.; and Plaat,
A., eds.,Computers and Games, 11–22. Berlin, Heidelberg:
Springer Berlin Heidelberg.
Wu, I.-C.; Lin, H.-H.; Sun, D.-J.; Kao, K.-Y .; Lin, P.-H.;
Chan, Y .-C.; and Chen, P.-T. 2013. Job-Level Proof Number
Search.IEEE Transactions on Computational Intelligence
and AI in Games, 5(1): 44–56.

---

<!-- page 10 -->

Sprouts Game
Sprouts is a well-known combinatorial pencil-and-paper
game introduced by Conway and Paterson, serving as the
domain for our experiments. Here, we provide an overview
of the game, including its rules, properties, and historical
background, and describe our efforts to solve its positions.
Rules of the Game
Sproutsstart withninitial spots arbitrarily placed on a sheet
of paper. The players then alternate in connecting the spots
by curves according to the following simple rules:
• Curves either connect two different spots or form loops
at a single spot.
• No curve can cross or touch itself or any other curve ex-
cept at the endpoints.
• Each spot can be incident to at most three curves; a loop
is counted twice.
• After a curve is drawn, the same player also places a new
spot along it.
• The first player who cannot make a move loses the game.
Figure 6 contains an example of a Sprouts game with two
initial spots. Here, the first player loses the game since the
last two spots in the final position, which are both incident to
less than three curves, cannot be connected without crossing
another curve.
(1) (2) (3)
(4) (5)
Figure 6: An example of a Sprouts game.
Sprouts is an impartial game played under the normal
play convention. In particular, it ends after a finite number
of moves, and there is no draw. It follows that there is a win-
ning strategy for either the first or the second player in every
position.
History and Related Work
Sprouts was designed by British mathematicians John Hor-
ton Conway and Michael Paterson in 1967 with the in-
tention to create a simple-to-play but difficult-to-analyze
game (Berlekamp, Conway, and Guy 2003; Roberts 2015).
Since the early beginnings of Sprouts, researchers have
tried to determine the outcomes ofn-spot positions for as
many values ofnas possible. Conway himself found the
outcomes forn≤3. Mollison later proved the outcomes for
nequal to 4 and 5, as well as forn= 6, whose proof by
hand took 49 pages (Gardner 1967).
Applegate, Jacobson, and Sleator (1991) created the first
computer solver for Sprouts, which proved the outcomes for
n≤11, and led the authors to formulate the Sprouts Conjec-
ture. Their solver used Alpha-Beta Pruning with the string
representation of positions on which all subsequent solvers,
including ours, were built. In 2006, Josh Purinton created
a computer program, calledAuntBeast, that played casual
games with players from theWorld Game Of Sprouts Asso-
ciationand was able to play a perfect game up to 14 spots;
thus, newly solving the game forn= 12,13,14.
The previous state-of-the-art solver GLOP, created by
Lemoine and Viennot (2015), solved alln-spot positions
forn≤32in 2007. In 2011, they extended the results for
n≤44plus three more values ton= 53; see Table 2. After
hearing about these achievements made by Lemoine and Vi-
ennot, Conway, one of the authors of Sprouts, reacted with
the following disbelieving words (Roberts 2015):
“I doubt that very much. They are basically say-
ing they have done the impossible. If someone says
they’ve invented a machine that can write a play wor-
thy of Shakespeare, would you believe them? It’s just
too complicated. If someone said they’d been hav-
ing some success teaching pigs to fly... Though if they
were doing that over in the field behind the Institute
[for Advanced Study in Princeton], I would like to
take a look. ”
The critical improvement by Lemoine and Viennot com-
pared to the previous solver by Applegate, Jacobson, and
Sleator (1991) stands on the utilization of Grundy num-
bers to analyze independent subpositions separately and on
human interventions to navigate the search outside of un-
promising parts of a game tree. Their algorithm originally
relied on Alpha-Beta Pruning with a heuristic for order-
ing child-node exploration. Later, Lemoine and Viennot re-
placed Alpha-Beta with a basic variant of Proof-Number
Search, enabling them to achieve several new results.
Solving Sprouts Positions
We derived 32 new outcomes with sequential SPOTS and 10
more new outcomes of much more complex positions with
parallel SPOTS. Thus, we extended the known outcomes of
n-spot positions by 42 new values in total, almost doubling
the number to the final 89 known values. Since the complex-
ity does not grow monotonically, as positions withn≡3
(mod 6)are particularly challenging, we established out-
comes for alln≤62and the27least complex cases with
63≤n≤104. For a detailed list of all the currently known
outcomes ofn-spot positions, see Table 2.
All the newly computed outcomes agree with the Sprouts
conjecture, which thus remains open. Additionally, we con-
firm that the computed Grundy numbers ofn-spot positions
follow the extended Sprouts conjecture of Lemoine and Vi-
ennot (2015), which states that the Grundy number of the
n-spot position withn= 0,1,or2 (mod 6)equals to 0,
and to 1 otherwise.

---

<!-- page 11 -->

n out by size n out by size n out by size n out by size n out by size
1 L C 2e0 22 W G 6e3 43 L G 1e6 64 W S 3e6 85 L S 2e7
2 L C 4e0 23 W G 4e3 44 L G 1e6 65 W S 1e7 86 L S 2e7
3 W C 7e0 24 L G 5e4 45 W S 3e6 66 — 87 —
4 W M 2e1 25 L G 2e4 46 W G 2e5 67 L S 1e7 88 W S 2e7
5 W M 3e1 26 L G 4e4 47 W G 2e5 68 L S 1e7 89 W S 1e9
6 L M 9e1 27 W G 3e5 48 L S 3e6 69 — 90 —
7 L A 2e2 28 W G 1e4 49 L S 3e6 70 W S 3e6 91 L S 2e7
8 L A 3e2 29 W G 1e4 50 L S 3e6 71 W S 1e7 92 L S 2e7
9 W A 1e1 30 L G 2e5 51 W S 1e7 72 — 93 —
10 W A 3e2 31 L G 5e4 52 W S 3e6 73 L S 1e7 94 W S 1e9
11 W A 2e2 32 L G 7e4 53 W G 8e5 74 L S 1e7 95 W S 1e9
12 L P 1e3 33 W G 1e6 54 L S 1e7 75 — 96 —
13 L P 1e3 34 W G 3e4 55 L S 3e6 76 W S 1e7 97 L S 1e9
14 L P 1e4 35 W G 3e4 56 L S 3e6 77 W S 1e7 98 L S 1e9
15 W G 9e4 36 L G 1e6 57 W S 1e9 78 — 99 —
16 W G 1e3 37 L G 1e5 58 W S 3e6 79 L S 1e7 100 W S 1e9
17 W G 6e2 38 L G 1e5 59 W S 7e5 80 L S 1e7 101 —
18 L G 7e3 39 W G 1e6 60 L S 1e9 81 — 102 —
19 L G 9e3 40 W G 1e5 61 L S 3e6 82 W S 1e7 103 L S 1e9
20 L G 1e4 41 W G 2e5 62 L S 3e6 83 W S 1e7 104 L S 1e9
21 W G 8e4 42 L G 1e6 63 — 84 — 105 —
Table 2: Solvedn-spot Sprouts positions. Each entry in thenth row indicates the solver (C = Conway, M = Mollison, A
= Applegate, Jacobson, and Sleator (1991), P = AuntBeast (Purinton 2006), G = GLOP (Lemoine and Viennot 2015), S =
SPOTS), the outcome (W = win, L = loss), and size of the proof measured by the number of computed Grundy numbers.
Lighter and darker G-entries indicate positions solved by GLOP in 2007 and in 2010–2011, respectively. Lighter and darker
S-entries indicate positions solved by the sequential and parallel versions of our SPOTS solver, respectively.
A server workstation equipped with an AMD EPYC 7302
processor (3.00 GHz, 32 cores) and 256 GB of RAM, run-
ning Debian GNU/Linux 12, was used for long-running
sequential computations. Massively parallel computations
were performed on theGoogle Compute Engineplatform,
part of theGoogle Cloud Platform, which enables the cre-
ation of a large number of interconnected virtual machines
(VMs). We set up a cluster composed of a single standard
VM for the master and multiple spot VMs for workers, with
automatic restoration after each preemption. All VMs were
located in theus-central1region and ran Ubuntu 22.04 LTS.
The standard VM used thec2d-highmem-32instance type,
providing 16 cores, 256 GB of RAM, and 256 GB of bal-
anced persistent disk. For workers, we allocated 16 spot
VMs of thec2-standard-60type, each with 30 cores, 240
GB of RAM, and 128 GB of balanced persistent disk.
After a month of computations on this cluster with 480
cores allocated to workers, we derived proofs of Sprouts
positions that are approximately 1,000 times larger than
the most complicated ones derived by the current state-of-
the-art solver GLOP (Lemoine and Viennot 2015; Lemoine
2011). We computed around10 9 Grundy numbers in total,
where one Grundy number corresponds to approximately
100 node expansions.
Notably, the results were achieved without fully utilizing
the potential of our solver, as parameters were not always
optimally tuned, and some workers were intermittently ter-
minated during computation due to spot machine preemp-
tion. The parallel SPOTS computations were executed on
480 CPU cores, primarily using the following parameter set-
tings:workers= 48,iterations= 10,000,updates
= 1,000,grouping= 3, andthreads= 10.
Code and Data.The project’s GitHub repository contains
the source code for our parallel Sprouts solver, SPOTS. It
also provides links to the Grundy number databases, which
serve as certificates for newly solved positions.
All computation-demanding parts of the solver, such as
the representation of a state and search algorithms, are im-
plemented in C++20 for performance reasons. The first-
level work distribution is implemented inPython3.10using
Ray2.47.0, a framework for distributed systems. The mas-
ter and the workers are coordinated using Ray, while they
utilize the core functions implemented in C++, exposed to
Python usingpybind112.10. The whole project is built us-
ingCMake3.22with the compilerGCC11.4.
The SPOTS solver can compute the outcome of any given
Sprouts position. Positions are provided as input using the
string representation introduced by Applegate, Jacobson,
and Sleator (1991) and later used by Lemoine and Viennot
(2015). In particular, then-spot position is encoded as0 *n.
Estimating the Game Tree Complexity
We describe how the estimates of the game tree complexity
in Figure 1 were obtained. For Checkers, Chess, Shogi, and
Go, we followed a standard approach based on known esti-
mates of the average branching factorband average depthd

---

<!-- page 12 -->

of the game tree. The complexity was then estimated asb d.
To estimate the mean and variance of game tree complex-
ity for Sprouts, we sampled 1,000 random games by select-
ing each child uniformly at random at each node along a path
Pfrom the root to a terminal node. Letb i denote the number
of children of theith node alongP. We computed Qk
i=1 bi
for each path, wherekis the number of internal nodes in
the path. The final estimate was then obtained by averaging
these values across all the samples, with the range between
the 1st and 99th percentiles as the measure of variance.
To estimate the complexity of the Sprouts tree with
Grundy numbers, we used a similar method based on ran-
domly sampled games, but with an important modification:
decomposable nodes must be treated separately, as their total
complexity is the sum of the complexities of subtrees of all
their children. The estimation is therefore recursive, and in-
stead of the pathPin the tree, we obtain a randomly sampled
subtreeT. As in our description of DFPN with GN, we first
simplify the Sprouts tree by replacing Grundy nodes with
atomic nodes, resulting in a tree that includes only atomic
and decomposable nodes. The subtreeTis then obtained by
starting at the root, selecting a single child at every encoun-
tered atomic node uniformly at random, and by selecting
all children of each encountered decomposable node. After
samplingT, we initially set the complexityC(t)of each ter-
minaltinTto be 1. To compute the complexityC(v)of the
Sprouts subtree of an internal nodevofT, we distinguish
two cases. Ifvis atomic, then we letC(v) =b(v)·C(c),
wherecis the child ofvinTandb(v)is the number of
children ofvin the Sprouts tree. For decomposablev, we
letC(v) = P
c C(c), where the sum is taken over all the
childrencofvin the Sprouts tree. The complexity estimate
of the Sprouts tree with Grundy numbers is then the average
of the valuesC(r)of the rootrtaken over 1,000 samples
ofT, with the range between the 1st and 99th percentiles
reported as the measure of variance. During the sampling,
we need to estimate the sizes of the Grundy numbers of the
nodes. However, these values are unknown. Using the av-
erage values of the Grundy numbers in the computed proof
trees ofn-spot positions, we estimate the number of relevant
children of Grundy nodes to determine their value.
Proof of Theorem 2
Here, we prove Theorem 2 by showing that our choice of
the thresholds in DFPN with GN guarantees that, for every
nodevofP, the MPN is in the subtree ofvif and only if the
following three inequalities are satisfied:
pt(v)> pn(v)
dt(v)> dn(v),
mt(v)>min{pn(v) +ps(v), dn(v) +ds(v)}.
(3)
We also provide some intuition for how the thresholds are
selected.
We first recall how the proof and disproof numbers are de-
fined. The proof and disproof numbers of an internal atomic
nodevare given by
pn(v) = min
c
dn(c)anddn(v) =
X
c
pn(c),(4)
where the minimum and the sum are taken over the children
cofv. Ifuis a Grundy node, then it corresponds to an atomic
positionQand we usev i to denote the child ofurepresent-
ingQ+∗i. Ifv j is the last child ofugenerated so far, then
we set
pn(u) =dn(u) = min{pn(v j), dn(vj)}.(5)
Letvbe a decomposable internal node. Then,vcorresponds
toP 1+···+P k, wherev′
k is an atomic node corresponding to
Pk +∗n⊕gn(P 1)⊕···⊕gn(P k−1)andu 1, . . . , uk denote
the nodes where eachu i is a Grundy node corresponding
toP i. The proof and disproof numbers ofv, are then given
by
pn(v) =dn(v) =
kX
i=1
pn(ui)(6)
ifv′
k has not yet been generated, and by
pn(v) =pn(v ′
k)anddn(v) =dn(v ′
k)(7)
otherwise.
Now, letvbe a node from the pathP, and letwbe the
next node to be selected. Ifvis atomic, then we letw ′ be
its child with the second-lowest disproof number. For any
nodev′, we usepn 0(v′)anddn 0(v′)to denote the values
ofpn(v′)anddn(v ′), respectively, before selectingwinv.
Since all updates of proof and disproof numbers go along the
pathPandwis the only child ofvonP, we always have
dn0(c) =dn(c)for every childc̸=wofv. Note that, by
combining this fact with (4), we get that an internal atomic
vsatisfies
dn0(v) =dn(v) +pn 0(w)−pn(w).(8)
Similarly, we obtain a relationship betweenpn 0(v)and
pn(v)for decomposablevusing (5). In particular, ifvis
decomposable andv′
k has not yet been generated, then
pn(v) =dn(v) =
X
c
min{pn(c), dn(c)}
=pn 0(v)−min{pn 0(w), dn0(w)}
+ min{pn(w), dn(w)}.
(9)
We also recall the definitions of the thresholds of nodes
inP. For atomicv, the thresholdspt(w),dt(w), andmt(w)
with parametersps(w)andds(w)are defined as
pt(w) =dt(v)−dn 0(v) +pn 0(w),
dt(w) = min{pt(v), dn 0(w′) + 1},
mt(w) =mt(v),
(10)
where the parametersps(w)andds(w)are set tops(w) =
ds(v) +dn 0(v)−pn 0(w)andds(w) =ps(v). If the node
vis decomposable, then we set
pt(w) =pt(v), dt(w) =dt(v), mt(w) =mt(v)(11)
ifv′
k has been generated. Ifv ′
k has not been generated yet,
we define
pt(w) =dt(w) =∞,
mt(w) =t(v)−pn 0(v) + min{pn 0(w), dn0(w)}, (12)

---

<!-- page 13 -->

where we letps(w) =ds(w) = 0andt(v) = min{pt(v),
dt(v), mt(v)−min{ps(v), ds(v)}}. For the rootr, we set
pt(r) =dt(r) =mt(r) =∞andps(r) =ds(r) = 0.
To gain some insight about the choice (13) of thresholds,
note that the definition (10) ofpt(w)anddt(w)for atomicv
is exactly the same as in DFPN without GN. The first value
indt(w)and the value ofpt(w)indicate that DFPN with GN
must backtrack when the subtree ofvno longer contains the
MPN. The second value indt(w)indicates that when the
MPN switches to the childw ′. The new thresholdmt(w)
is needed because it follows from (9) that to havept(v)>
pn(v)anddt(v)> dn(v)for decomposablevwe need an
upper bound onmin{pn(w), dn(w)}, which is eventually
ensured bymt(w)in combination withps(w)andds(w),
which serve for shifting the values in the atomic nodev.
Proof of Theorem 2.We proceed by induction on the depth
dof a nodevinPand prove that MPN is in the subtree of
vif and only if the inequalities (3) are satisfied. Sincevis
inP, it is an internal node. We note that the MPN is in the
subtree ofvif and only if all nodesv ′ on the subpath ofP
from the root tovsatisfy the following condition: ifv ′ has
atomic parentp, thenv ′ has the smallest disproof number
among all children ofp.
For the induction base, we haved= 0, in which case
our node is the rootr. Ifrhas not yet been solved, then the
MPN is always in the subtree ofr, and the inequalities (3)
are satisfied aspt(r) =∞> pn(r),dt(r) =∞> dn(r),
andmin{pn(r) + 0, dn(r) + 0}<∞=mt(r). Ifris
solved, then obviously the MPN is not in the subtree ofr,
and the threshold inequalities are not satisfied, since∞<∞
is interpreted as false.
For the induction step, we assumed≥1and prove that
the MPN is in the subtree of the next selected nodewif and
only if
pt(w)> pn(w)
dt(w)> dn(w),
mt(w)>min{pn(w) +ps(w), dn(w) +ds(w)}.
(13)
We assume that the claim is true for the parentvofw. Note
that the parentvexists asd≥1.
Assume first that the MPN is in the subtree ofw. We show
that the three inequalities (13) are satisfied. Sincewis a child
ofv, the subtree ofwis contained in the subtree ofv, and
therefore the MPN is in the subtree ofv. It then follows from
the induction hypothesis that the inequalities (3) are satis-
fied.
We now distinguish two cases. First, we assume thatvis
atomic. Since the MPN is in the subtree ofw, the nodew
has the smallest disproof number among all children ofv. In
particular,dn(w)≤dn(w ′) =dn 0(w′), because the MPN
is in the subtree ofw.
To prove the first inequality in (13),pt(w) =dt(v)−
dn0(v) +pn 0(w)by (10). Substituting fordn 0(v)using (8),
we rewrite this equality aspt(w) =dt(v)−dn(v) +pn(w).
Sincedt(v)> dn(v)by (3), we havept(w)> pn(w),
which satisfies the first inequality of (13).
Second, we usedt(w) = min{pt(v), dn 0(w′)+1}, which
holds by (10). Usingdn(w)≤dn(w ′) =dn 0(w′)and
pt(v)> pn(v), which follows from (3), we getdt(w)≥
min{pn(v) + 1, dn(w) + 1}. Sincevis atomic, we have
pn(v)≥dn(w)by (4). Altogether, this gives the second
inequalitydt(w)> dn(w)from (13).
To prove the last inequality of (13), we first show that
mt(w) =mt(v)and thatmin{pn(v) +ps(v), dn(v) +
ds(v)}= min{pn(w) +ps(w), dn(w) +ds(w)}. First, we
havemt(w) =mt(v)by (10). Also,ps(w) =ds(v) +
dn0(v)−pn 0(w), which becomesps(w) =ds(v)+dn(v)−
pn(w)after substituting fordn 0(v)using (8). So, we get
pn(w) +ps(w) =dn(v) +ds(v). Moreover, we have
ds(w) =ps(v)and sodn(w) +ds(w) =dn(w) +ps(v).
Using the definition (4) ofpn(v), we getpn(v) =dn(w),
sincewhas the smallest disproof numbers among all chil-
dren of the atomic nodev, and so we get the desired equal-
itydn(w) +ds(w) =pn(v) +ps(v). Thus, we indeed
havemt(w) =mt(v)andmin{pn(v) +ps(w), dn(w) +
ds(w)}= min{pn(w) +ps(w), dn(w) +ds(w)}. Then we
are done, as the third inequality (3) is equivalent to the third
inequality in (13).
Now, assume thatvis decomposable. We distinguish two
subcases, depending on whether the last childv ′
k ofwhas
been generated.
Assume first thatv′
k has been generated. Thenw=v ′
k by
the choice of the selected node in the algorithm. According
to (7), we then havepn(v) =pn(w)anddn(v) =dn(w).
Moreover,pt(v) =pt(w),dt(v) =dt(w), andmt(v) =
mt(w)by (11). Since all values are the same forvandw, all
inequalities from (13) follow immediately from (3).
For the other subcase, assume thatv ′
k has not yet been
generated. Then, the first two inequalities in (13) are triv-
ially satisfied, aspt(w) =dt(w) =∞whilepn(w)and
dn(w)are finite, since the subtree ofwcontains the MPN.
To prove the third inequality in (13), it suffices to show that
mt(w)>min{pn(w), dn(w)}, asps(w) =ds(w) = 0for
decomposablev. We recall that
mt(w) =t(v)−pn 0(v) + min{pn 0(w), dn0(w)},
where
t(v) = min{pt(v), dt(v), mt(v)−min{ps(v), ds(v)}}.
We now prove thatt(v)> pn(v) =dn(v). From (6),
we obtainpn(v) =dn(v). Using (3), we estimate the
first two terms int(v)from below aspt(v)> pn(v)
anddt(v)> dn(v) =pn(v). To estimate the third term
mt(v)−min{ps(v), ds(v)}int(v)withpn(v), we first ex-
pandmt(v)as
mt(v) = min{pn(v) +ps(v), dn(v) +ds(v)}
=pn(v) + min{ps(v), ds(v)}
where we usedpn(v) =dn(v). The termmin{ps(v),
ds(v)}then cancels out int(v)and we indeed obtaint(v)>
pn(v).
Usingt(v)> pn(v), we now have
mt(w)> pn(v)−pn 0(v) + min{pn 0(w), dn0(w)}.

---

<!-- page 14 -->

To finish the proof ofmt(w)>min{pn(w), dn(w)}, we
expresspn(v)in terms of (9), obtaining
mt(w)> pn 0(v)−min{pn 0(w), dn0(w)}
+ min{pn(w), dn(w)−pn 0(v)}
+ min{pn0(w), dn0(w)}
= min{pn(w), dn(w)−pn 0(v)},
which finishes the proof of the last inequality in (13).
For the other implication, assume that the inequalities (13)
are satisfied. We again distinguish two cases depending on
the type ofv.
First, we assume thatvis atomic. We aim to show that
the MPN lies in the subtree ofw. To do this, it suffices to
show that the inequalities (3) hold, as this implies, by the
induction hypothesis, that the MPN is in the subtree ofv.
Sincewis the selected node, it is the child ofvwith the
smallest disproof number at the time of its selection. That is,
dn0(w) = min c dn0(c)where the minimum is taken over
all childrencofv. We want to show thatwis also the child
ofvwith the smallest disproof number at all steps where
wisP, as then the MPN indeed lies in the subtree ofw.
This follows from the definition (10) ofdt(w), which im-
pliesdt(w)≤dn 0(w′) + 1, and thusdn(w)< dt(w)≤
dn0(w′) + 1 =dn(w ′) + 1, where we usedn 0(c) =dn(c)
for every childc̸=wovand the fact thatw ′ had the second-
smallest disproof number among the children ofvat the time
of selectingwas the next node.
To prove the first inequality in (3), the definition (10) of
dt(w)givesdt(w) = min{pt(v), dn 0(w′) + 1}. In partic-
ular,pt(v)≥dt(w). We also havedn(w)≥pn(v), which
follows from (4) and from the fact thatwis a child ofv.
By the second inequality in (13), we then obtainpt(v)≥
dt(w)> dn(w)≥pn(v), which implies the first inequality
in (3).
Second, we expand the definition (10) ofpt(w)to ob-
tainpt(w) =dt(v)−dn 0(v) +pn 0(w). Using (8), we
rewrite this equality aspt(w) =dt(v)−dn(v) +pn(w). By
the first inequality in (13), we then havept(w) =dt(v)−
dn(v) +pn(w)> pn(w). Equivalently, the second inequal-
itydt(v)> dn(v)in (3) is true.
Finally, to prove the third inequality in (3), we first use the
same arguments as in the proof of the first implication, and
we derivemt(w) =mt(v)andmin{pn(v)+ps(v), dn(v)+
ds(v)}= min{pn(w) +ps(w), dn(w) +ds(w)}. The third
inequality in (13) is then equivalent to the third inequality
in (3), and we are done.
Now, assume thatvis decomposable. We still aim to show
that the MPN lies in the subtree ofw. Again, it suffices to
show that the inequalities (3) hold. The induction hypothesis
then implies that the MPN is in the subtree ofvand, since the
selected nodewis the only generated child ofvthat has not
yet been solved, we see that the MPN is in the subtree ofw.
We again distinguish two subcases, depending on whether
v′
k has been generated or not.
Assume first thatv′
k has been generated. Then, as before,
all values in (3) and (13) are the same, and thus all the in-
equalities in (3) are satisfied.
For the other subcase, we assume thatv′
k has not yet been
generated. We prove all three inequalities in (3) at the same
time. The third inequality in (13) gives the following esti-
mate:
mt(w)>min{pn(w) +ps(w), dn(w) +ds(w)}
= min{pn(w), dn(w)},
where we usedps(w) =ds(w) = 0for decomposablew.
We now expand the definition ofmt(w)as follows:
mt(w) =t(v)−pn 0(v) + min{pn 0(w), dn0(w)},
where
t(v) = min{pt(v), dt(v), mt(v)−min{ps(v), ds(v)}}.
Using (9), the expression ofmt(w)becomes
mt(w) =t(v)−pn(v)−min{pn 0(w), dn0(w)}
+ min{pn(w), dn(w)}+ min{pn 0(w), dn0(w)}
=t(v)−pn(v) + min{pn(w), dn(w)}.
Therefore, we know that
t(v)−pn(v) + min{pn(w), dn(w)}>min{pn(w), dn(w).
This can be simplified ast(v)> pn(v). Thus, using the
expression oft(v), we derive
min{pt(v), dt(v), mt(v)−min{ps(v), ds(v)}}> pn(v).
Sincepn(v) =dn(v)by (6), this implies the first two in-
equalities in (3). For the third inequality, we obtained that
mt(v)> pn(v) + min{ps(v), ds(v)}.
However, sincepn(v) =dn(v), we can rewrite this as
mt(v)>min{pn(v) +ps(v), dn(v) +ds(v)},
obtaining the last inequality in (3).
This finishes the induction step and the proof of Theo-
rem 2.
Parameter Analysis
We begin by recalling the final scaling efficiency of PNS-
PDFPN with optimally tuned parameters, as shown in Fig-
ure 7, where we also include a logarithmic scale for a more
detailed evaluation. In this section, we analyze the optimal
parameter settings of PNS-PDFPN by examining thesearch
overhead, defined as the ratio of node expansions performed
by PNS-PDFPN to those performed by a sequential DFPN.
To capture the synchronization and communication over-
head resulting from frequent interactions with the master,
we also measure theworker utilization, defined as the aver-
age proportion of time that workers are actively assigned to
a job.
A cluster consisting of 25 nodes, each equipped with 2×
AMD EPYC 9474F processors (3.60 GHz, 48 cores per pro-
cessor), 1536 GB of RAM, and 2× 7 TB NVMe drives, in-
terconnected via 10 Gbit/s Ethernet, was used for distributed
experiments. An exhaustive search over all reasonable pa-
rameter settings of PNS-PDFPN was performed using up to
512 CPU cores. Due to limited access to the cluster, only the
most promising experiments were computed for the 1024-
core setting. To ensure reproducibility of the random sam-
pling used for breaking ties in child selection, random seeds
1, 2, and 3 were fixed.

---

<!-- page 15 -->

1 2 4 8 16 32 64 128 256 5121024
CPU Cores
0
100
200
300Speedup Relative to DFPN(5 · 107)
 PDFPN
PNS-DFPN
+ GN Sync
+ GN Sync + Gr
+ GN Sync + Gr + Th
(our PNS-PDFPN)
PNS-PDFPN + Hr
1 2 4 8 16 32 641282565121024
CPU Cores
1
2
4
8
16
32
64
128
256
512
1024Speedup Relative to DFPN(5 · 107)
Figure 7: Impact of GN synchronization (GN Sync), grouping (Gr), second-level parallelization (Th), and the child-ordering
heuristic (Hr) on the final speedup of PNS-PDFPN relative to DFPN(5·10 7). Measured on the 47-spot position. The semi-log
scale, with a logarithmicx-axis and a lineary-axis, is shown on the left; the log-log scale, with both axes logarithmic, is shown
on the right.
Analysis of Retention and Sharing
In Figure 8, we revisit the comparison between PNS-PNS
and PNS-DFPN with varying levels of state retention and
key result sharing (Grundy numbers, in the case of impar-
tial games). We observe that the poor speedup of PNS-PNS
without Grundy number retention is primarily due to sub-
stantial search overhead, as workers repeatedly recompute
results that they have discarded before. The utilization of
workers is also slightly lower than in the other settings. We
attribute this to increased traffic between workers and the
master, as the workers are less able to solve the children of
the assigned leaf nodeℓimmediately. As a result, they send
back a larger number of unproven children, which must then
be expanded in the master tree.
Retaining Grundy numbers already leads to a substantial
reduction in search overhead, resulting in improved speedup.
The same effect is observed when proof and disproof num-
bers are also retained by using DFPN workers at the second
level. The search overhead is further reduced when all de-
rived Grundy numbers are shared among workers, without
compromising worker utilization. This highlights the suit-
ability of Grundy numbers as key results to share, as they are
inexpensive to distribute and widely reusable. Thus, PNS-
DFPN with Grundy number synchronization achieves supe-
rior scaling efficiency compared to its unsynchronized vari-
ant, a difference that becomes even more pronounced for the
larger position shown in Figure 7.
Also, note that beyond a certain point, adding more work-
ers to the algorithm stops being beneficial, as the overhead
on the master grows significantly. This is evident from the
low utilization of the workers. In the following subsection,
we explain how to address this limitation.
Optimal Parameter Settings
We analyze the setting of the following four parameters:
iterations, the maximum number of expansions per
job;updates, the number of iterations between each up-
date sent to the master;grouping, the number of workers
grouped within a single node; andthreads, the number of
threads assigned to PDFPN per worker.
Updates.First, in Figure 9, we show how setting the num-
ber ofupdatesin PNS-PDFPN with GN synchroniza-
tion, no grouping, and no second-level parallelization affects
the scaling. We observe that a lower number ofupdates
naturally reduces worker utilization, since more frequent
synchronization of Grundy numbers and tree updates in-
crease the overhead on the master. On the other hand, with
smaller values ofupdates, the search overhead increases
at a slower rate, as the workers are more synchronized due
to more frequent GN synchronization. Therefore, a rea-
sonable trade-off must be made between worker utilization
and search overhead. Also, notice that increasingupdates
above 1,000 does not increase the worker utilization, which
causes the drop in speedup. Hence, settingupdatesto
1,000 almost always achieves the best scaling.
Iterations.When theupdatesparameter is fixed, the to-
tal job length can be optimized by adjusting the
iterationsparameter; see Figure 9. With longer jobs,
workers switch less frequently between potentially very
different parts of the tree, allowing them to better uti-
lize locally stored proof and disproof numbers. However, a
more focused search leads to less exploration of the mas-
ter tree, which may result in computations in suboptimal
parts of the tree. Therefore, we observe that larger values
ofiterationsbenefit PNS-PDFPN when using a large

---

<!-- page 16 -->

1 2 4 8 16 32 64 128 256 512
CPU Cores
25
26
27
28
29
210
211
212
213
Computation Time [s]
1 2 4 8 16 32 64 128 256 512
CPU Cores
0
25
50
75
100
125
Search Overhead
1 2 4 8 16 32 64 128 256 512
CPU Cores
0
20
40
60
80
100
Worker Utilization
DFPN(106) PNS-PNS PNS-PNS + GN PNS-DFPN + GN PNS-DFPN + GN Sync
Figure 8: Comparison of PNS-PNS and PNS-DFPN, including the impact of retention of Grundy numbers (GN) in workers
and their synchronization (GN Sync). All variants are run on the 29-spot position with 100iterations, 100updates, no
grouping, and no second-level parallelization.
number of workers, as a sufficient number of workers can
guarantee sufficient exploration. The opposite effect is vis-
ible with a low number of workers, where short-term jobs
achieve better scaling.
Grouping.As previously observed, beyond a certain po-
int, adding more workers no longer accelerates the algorithm
due to excessive overhead on the master. One way to reduce
overhead and improve worker utilization is to group work-
ers located on the same node. This allows workers to share
the local database of key results (Grundy numbers), which
reduces memory consumption and allows instant synchro-
nization of key results between workers. This slightly de-
creases the search overhead as shown in Figure 10, where
we setgroupingto 32 (note that the size ofgroupingis
limited by the number of cores of a node). Moreover, fewer
redundant key results derived in parallel by multiple workers
are shared, which reduces overhead on the master and con-
sequently improves worker utilization. Therefore, grouping
workers enables PNS-PDFPN to scale efficiently with up to
at least 1024 CPU cores.
Threads.Finally, in Figure 11, we examine how the pa-
rameterthreads, responsible for second-level paralleliza-
tion, affects scaling efficiency. In general, similar to config-
uring theiterationsparameter, stronger second-level
parallelization is beneficial only when a larger number of
CPU cores is used, as there are already enough workers to
ensure sufficient exploration of nodes in the master tree. This
effect is even stronger if the child-ordering heuristic is used
to select the child in the case of tied disproof numbers. In this
case, the algorithm can reduce the exploration even more
and rely on the heuristic by further increasing the values of
iterationsandthreads, resulting in a more localized
search; recall Table 1, which shows the optimal parameter
settings with and without the heuristic.
Complete Statistics of PNS-PDFPN Scaling
In Table 3, we present statistics collected for PNS-PDFPN
with the child-ordering heuristic after applying the optimal
parameter settings, extending Table 1. We measured the fol-
lowing seven values on the 47-spot position: the number of
nodes in the master tree (M-Nodes), the average number of
nodes in workers’ transposition tables (W-Nodes), the to-
tal number of computed Grundy numbers (T-GN), the total
number of iterations (T-It), the worker utilization (Util), the
computation time (Time), and the measured speedup.
We observe that worker utilization remains above99%in
all measured instances, up to 1024 CPU cores. The search
overhead increases slightly, from approximately 0.94 with
1 core to 2.50 with 1024 cores. The speedup continues to
grow, reaching332.97±26.80at 1024 cores. Measurements
were made with respect to DFPN(5·10 7) as the baseline.
Note that reporting the speedup of PNS-PDFPN with respect
to PNS-PDFPN with 1 core almost does not change the re-
ported speedup, since both PNS-PDFPN and DFPN have al-
most identical running times.
We believe that the algorithm can scale well even beyond
1024 cores, as PNS-PDFPN seems to scale effectively as
long as second-level parallelization can be increased. This
may be feasible, given that PDFPN scales well on up to 64
cores.

---

<!-- page 17 -->

1 2 4 8 16 32 64 1282565121024
0.5
1
2
4
8
16
32
64
128
256
512
1024
Speedup Relative to DFPN(5 · 107) Updates = 100
1 2 4 8 16 32 64 1282565121024
Updates = 1,000
1 2 4 8 16 32 64 1282565121024
Updates = 10,000
1 2 4 8 16 32 64 1282565121024
2
4
6
8Search Overhead
1 2 4 8 16 32 64 1282565121024
1 2 4 8 16 32 64 1282565121024
1 2 4 8 16 32 64 1282565121024
CPU Cores
0
20
40
60
80
100Worker Utilization
1 2 4 8 16 32 64 1282565121024
CPU Cores
1 2 4 8 16 32 64 1282565121024
CPU Cores
Iterations = 100 Iterations = 1,000 Iterations = 10,000 Iterations = 100,000
Figure 9: Analysis of the impact ofiterationsandupdatessettings on the scaling of PNS-PDFPN with GN synchro-
nization, no grouping, and no second-level parallelization.

---

<!-- page 18 -->

1 2 4 8 16 32 64 1282565121024
CPU Cores
0.5
1
2
4
8
16
32
64
128
256
512
1024
Speedup Relative to DFPN(5 · 107)
1 2 4 8 16 32 64 1282565121024
CPU Cores
2
4
6
Search Overhead
1 2 4 8 16 32 64 1282565121024
CPU Cores
0
20
40
60
80
100
Worker Utilization
No Grouping Grouping
Figure 10: Analysis of the impact ofgroupingsettings on the scaling of PNS-PDFPN with GN synchronization and no
second-level parallelization.Groupingis set to its maximum (up to 32) andupdatesto 1,000.
Cores Parameters PNS-PDFPN + Heuristic
It Th M-Nodes W-Nodes T-GN T-It Util Time [s] Speedup
DFPN — — 15M±1.6M— 268k±30k17.2M±1.9M— 17.6k±2.5k—
1 1k 1 609k±0.014.3M±0.0199k±0.016.3M±0.099.8±0.017.9k±4.290.99±0.00
2 1k 1 500k±56k6.5M±665k159k±15k14.4M±1.5M99.7±0.07.67k±8722.36±0.28
4 1k 1 540k±40k3.7M±303k152k±13k16.2M±1.3M99.7±0.04.11k±2854.34±0.32
8 100k 1 64k±7701.8M±11.8k130k±4.0k16.0M±0.2M99.7±0.02.19k±24.98.06±0.09
16 100k 2 6.4k±2230.9M±35.6k150k±6.1k17.6M±0.7M99.7±0.01.09k±34.916.22±0.53
32 100k 1 1.9k±2941.5M±231k93.8k±17k16.3M±2.5M99.7±0.0707±11226.13±3.87
64 100k 2 1.5k±0.31.5M±57.5k90.7k±3.2k18.5M±0.7M99.7±0.0432±12.240.86±1.13
128 100k 4 3.3k±1.20.5M±5.63k90.2k±1.3k20.9M±0.2M99.7±0.0202±1.9087.02±0.81
256 100k 8 3.3k±120.6M±41.2k122k±6.5k30.6M±2.0M99.6±0.0155±11.1115.11±8.88
512 100k 16 3.3k±1.20.6M±22.8k112k±6.9k32.9M±1.3M99.5±0.087.3±4.43203.05±9.82
1024 100k 32 3.2k±0.30.6M±49.1k119k±13k43.0M±3.2M99.2±0.053.7±4.69332.97±26.8
Table 3: Statistics measured for PNS-PDFPN with the child-ordering heuristic after applying the optimal parameter settings.
The measured values are the number of nodes in the master tree (M-Nodes), the average number of nodes in workers’ trans-
position tables (W-Nodes), the total number of computed Grundy numbers (T-GN), the total number of iterations (T-It), the
worker utilization (Util), the computation time (Time), and the measured speedup. We also include the value of the parameters
iterations(It) andthreads(Th) and measurement for DFPN(5·10 7).Groupingset to its maximum andupdatesto
1,000 are optimal across all settings. Workers’ transposition table capacity is5·10 7. Measured on the 47-spot position.

---

<!-- page 19 -->

(a) Without heuristic
1 2 4 8 16 32 64 1282565121024
CPU Cores
0.5
1
2
4
8
16
32
64
128
256
512
1024
Speedup Relative to DFPN(5 · 107) Iterations = 1,000
1 2 4 8 16 32 64 1282565121024
CPU Cores
Iterations = 10,000
1 2 4 8 16 32 64 1282565121024
CPU Cores
Iterations = 100,000
(b) With heuristic
1 2 4 8 16 32 64 1282565121024
CPU Cores
0.5
1
2
4
8
16
32
64
128
256
512
1024
Speedup Relative to DFPN(5 · 107) Iterations = 1,000
1 2 4 8 16 32 64 1282565121024
CPU Cores
Iterations = 10,000
1 2 4 8 16 32 64 1282565121024
CPU Cores
Iterations = 100,000
Threads = 1 Threads = 2 Threads = 4 Threads = 8 Threads = 16 Threads = 32
Figure 11: Analysis of the impact ofthreadssettings on the scaling of PNS-PDFPN with GN synchronization. Two variants
of PNS-PDFPN are evaluated: one without the child-ordering heuristic (a) and one with it (b).Groupingis set to its maximum
(up to 32).
