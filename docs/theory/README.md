# Theory library

Vendored research papers: one directory per paper, `<slug>/` containing
the original `<slug>.pdf` and a full-text extraction `<slug>.md`.
Slugs are `<algorithm-or-system-name>-<year>`. Extraction headers record
provenance (publication, DOI where available, extraction method and known
defects) and point to the mining analyses under `docs/plans/`, which
remain the initiative-owned analytical layer — this library is only the
raw corpus.

Cross-references: `definitions.md` (PV/PPV/SPPV) and `proof.md` define
the proof-certificate vocabulary used across the extractions and mining
files.

| Directory | Paper | Venue | Mined in |
|---|---|---|---|
| `pdfpn-2010/` | Kaneko, *Parallel Depth First Proof Number Search* | AAAI-10, pp. 95–100 | `docs/plans/dfpn/research_parallel.md` |
| `ghi-2004/` | Kishimoto & Müller, *A General Solution to the Graph History Interaction Problem* | AAAI-04, pp. 644–649 | `docs/plans/dfpn/research_ghi.md` |
| `ghi-journal-2005/` | Kishimoto & Müller, *A solution to the GHI problem for depth-first proof-number search* | Information Sciences 175(4), 2005 | `docs/plans/dfpn/research_ghi_journal.md` |
| `epsilon-trick-2007/` | Pawlewicz & Lew, *Improving Depth-first PN-Search: 1 + ε Trick* | CG 2006, LNCS 4630 | `docs/plans/dfpn/research_epsilon.md` |
| `deep-pns-2015/` | Ishitobi et al., *Reducing the Seesaw Effect with Deep Proof-Number Search* | ACG 2015, LNCS 9525 | (see `deep-pns-2015/deep-pns-2015.md`) |
| `solrex-2016/` | Young & Hayward, *A Reverse Hex Solver* | CG 2016 / arXiv:1707.00627 | `docs/plans/parallel/research_solrex.md` |
| `deep-dfpn-2017/` | Zhang, Iida, van den Herik, *Deep df-pn and Its Efficient Implementations* | ACG 2017, LNCS 10664 | (see `deep-dfpn-2017/deep-dfpn-2017.md`) |
| `ppn2-2011/` | Saffidine, Jouandeau, Cazenave, *Solving Breakthrough with Race Patterns and Job-Level Proof Number Search* | ACG 2011, pp. 196–207 | `docs/plans/parallel/research_jlpns.md` |
| `pns-pdfpn-2025/` | Čížek, Balko, Schmid, *Massively Parallel Proof-Number Search for Impartial Games and Beyond* | arXiv:2511.10339 / AAAI-26 | `docs/plans/parallel/research_cizek2025.md` |

Status of each entry (Open / Cited / Mined) is tracked centrally in
`docs/bibliography.md`; this index only records where things live.

Conventions:

- PDF originals live only here — `docs/plans/` mining files reference
  them via these paths and never vendor their own copies.
- Extractions are faithful text extractions with a provenance blockquote;
  they are not summaries. Analysis lives in the mining files.
- New papers: create `<slug>/<slug>.pdf` + `<slug>/<slug>.md`, add a row
  here, and register the entry in `docs/bibliography.md`.
