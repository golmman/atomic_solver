# plan9 measurements — operator validation

Provenance: one 4-racer m20_white operator-validation race of the new
`examples/portfolio` driver (plan9 stage 3; cheap, non-gated — the plan8
campaign record stands as the lever's evidence).

| file | what |
| --- | --- |
| `env.json` | environment + race configuration |
| `state/m20_white_race.json` | parsed race result + anchor check |
| `logs/` | raw driver transcript (gitignored, never committed) |

Anchor verdict: **exact** — winner salt 5 at 791,401,553 evals (plan8
campaign anchor), `win`, exit 0. Wall 64.3 s (timing-dependent by design;
plan8 W_first median was 95.89 s under heavier ambient load). The
stop-when-proven kill policy held: three racers killed at first-decisive,
race cost ≈ 248 core-s vs 747 with plan8's 240 s grace.

A cheap dec10 4-racer race was additionally run pre-gate during driver
development (winner salt 2, 2,918,636 evals — the plan8 dec10 anchor,
exact) and is permanently encoded as the `dec10_race_reproduces_plan8_winner_anchor_exactly`
smoke test in `tests/portfolio_smoke.rs`.
