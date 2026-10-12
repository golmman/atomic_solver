# plan10 measurements — cooperative portfolio (cross-racer solved-fact sharing)

Provenance: plan10 stage 0–4. Stage 0 shipped the mechanism (`--facts-out`
/ `--facts-in`, `src/facts/`, the `search::dfpn::FactImporter` hook) with a
clean drift gate; stage 1 proved export-inertness; stage 2 ran the
pre-registered canary (clean — the A2 Loss-only fallback never fired);
stage 3 ran the pre-registered campaign (5 rounds × 6 cases, A0/A1
interleaved); stage 4 is `analyze_portfolio10.py` → `state/races10.json`.

| file | what |
| --- | --- |
| `env.json` | environment + campaign configuration |
| `races10.py` | race driver: 4 racers, stop-when-proven, facts plumbing, per-race state |
| `campaign10.py` | stage-3 campaign loop (pre-registered table, resumable) |
| `solo_anchors10.py` | fresh same-binary salt-0 solo anchors (H4a references) |
| `analyze_portfolio10.py` | stage-4 analysis, H1–H4 evaluated mechanically |
| `state/*.json` | per-race records (`<case>_<arm>_r<N>.json`), canary races (`*canary*`), `solo_s0.json`, aggregate `races10.json` |
| `logs/` | raw racer transcripts + per-race facts dirs (gitignored, never committed) |

Headline (details in `report10.md`): verdict **reject** per the
pre-registered rule — H2 fails on 5/6 cases. The failure channel is the
facts-plumbing wall tax (~1.2–1.4× `W_first` at ≈1.0× `evals_first`), not a
search regression; H1 fired exactly where predicted (m20_white 0.66×
evals / 0.82× wall) and the mechanism stayed sound everywhere (zero
outcome conflicts, zero CRC failures, canary clean, pure-R0 fidelity
exact). One winnerless m20 A1 round (all four racers budget-censored)
recorded as the chaos-tail risk.
