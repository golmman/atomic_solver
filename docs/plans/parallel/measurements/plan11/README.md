# plan11 measurements — low-tax solved-facts carrier (key-only emission, chunk-boundary import, near-root admission filter)

Provenance: plan11 stage 0–4. Stage 0 reworked `src/facts/` (record v2 +
`PATH_LEN_CAP=24` export filter, synchronous `FactsExporter`, threadless
chunk-boundary `FactsImporter`; threads/queue/A2 arm deleted) with a clean
drift gate; stage 1 measured the solo tax (M1) and facts volume; stage 2 ran
the pre-registered canary (clean); stage 3 ran the pre-registered campaign
(5 rounds × 6 cases, A0/A1 interleaved); stage 4 is
`analyze_portfolio11.py` → `state/races11.json`.

| file | what |
| --- | --- |
| `env.json` | environment + campaign configuration (incl. the tmpfs facts-location finding) |
| `races11.py` | race driver: 4 racers, stop-when-proven, facts plumbing (no A2 arm), per-race state |
| `probes11.py` | stage-1 M1 paired probes + facts-volume probe (facts files on tmpfs) |
| `solo_anchors11.py` | fresh same-binary salt-0 solo anchors (M2/H4a references) |
| `campaign11.py` | stage-3 campaign loop (pre-registered table, resumable) |
| `analyze_portfolio11.py` | stage-4 analysis, M1/M2/H1–H4 evaluated mechanically |
| `state/probes11.json` | stage-1 results + `_m1` verdict |
| `state/probes11_overlayfs_confound.json` | the confounded first stage-1 pass (facts files on the overlay-backed repo tree) — kept as the diagnosis record |
| `state/*.json` | per-race records, canary races, `solo_s0.json`, aggregate `races11.json` |
| `logs/` | raw racer transcripts + campaign log (gitignored, never committed); per-race facts dirs live under `/tmp/plan11_facts` |

Headline (details in `report11.md`): verdict **reject** per the
pre-registered rule — M2 (in-race tax ≤ 1.06) fails on stress (1.141) and
m22 (1.110), and H2 fails on stress (1.172/1.158) and dec10 (1.152/1.088).
M1 passed decisively (solo ratios 0.995–1.006 vs ≤ 1.03 — the solo tax is
gone), H1 fired on m20_white (0.575× evals / 0.601× wall, with one
winnerless round), H3/H4 were clean. Diagnosis: the M2 excess is the
environment's 4-way contention floor, not the carrier — fresh A0-control
rounds show the *pure facts-free* R0 at 1.24–1.25× its solo wall on dec13,
so the pre-registered ceiling was miscalibrated for the current binary; the
H2 failures are the TT-pollution channel (imports perturbing winning
basins), not wall tax. The M1 stage also exposed that plan10's solo
stage-1 tax was mostly the same overlay-filesystem confound.
