# plan9 measurements (ordering-guidance seed spike, Phase 0)

Provenance: HEAD `b4d9709` (post `parallel` plan6), aarch64 container,
release build. Fairy-Stockfish binary `libs/Fairy-Stockfish/src/stockfish`,
sha256 `cf65ef2f30ba0026922214928aa4899175611d282a49cc6ba2a88010df60309b`
(the plan2 armv8 build; rebuilt queries only, no recompile needed).

Case FENs: stress = m21_white = `make stress`
`4r2k/3p4/2pB2p1/p4p1p/7P/2N1PPP1/P1PP4/1R4RK w - - 0 21`;
m22 = `4r2k/3p4/2pB2p1/p6p/5pPP/2N1PP2/P1PP4/1R4RK w - - 0 22`;
dec controls = dec01/dec10/dec13 (the three hardest dec cases by
plan8 quick@0.125 child_evals: 5.71M / 4.26M / 3.82M).

## Files

| file | content |
|---|---|
| `baselines.json` | step 0: per case (stress FO+default, m22 FO, dec01/10/13 FO) wall, nodes, PV, pv_status, benchmark child_evals |
| `bench_baselines.json` | step 0: benchmark JSON rows (name/mode/evals/nodes/time/pv_len) |
| `move_order_probe.txt` | step 0: full static root-ordering probe, both cases (move_order_debug) |
| `sf_battery.json` | step 1: engine query battery (nodes 1M/10M, K=1; stress K=3; determinism run 2) incl. full PVs and engine walls |
| `sf_query.py` | step 1 driver: stdio UCI query script |
| `spike_root_work.json` | step 3: root-work attribution tables (unguided stress FO/default, guided runs incl. stress FO full per-move table and per-run summaries with unguided baselines) |

All solver stdout/stderr transcripts and the spike instrumentation itself were
regenerable and are not committed (measurement convention); the parsed JSON
plus the committed driver reproduce every number quoted in `report9.md`.

## Identity anchors

- Step 0 reproduction: stress FO 249,480,478 / default 338,094,183;
  m22 FO 14,156,269; dec13 3,822,602; dec10 4,262,128 — all exact.
- Spike non-perturbation: spike-off vs spike-on stdout md5 identical
  (stress FO `cfc58bc4419790c9a2f5972d7594302c`, m22 FO
  `8109ff0db5a03d1e95b20b9f8fe6a605`); spike totals equal baseline evals.
- Post-revert stress FO stdout md5 `cfc58bc4419790c9a2f5972d7594302c`
  (byte-identical to the in-session HEAD capture; `git diff src/` empty).
