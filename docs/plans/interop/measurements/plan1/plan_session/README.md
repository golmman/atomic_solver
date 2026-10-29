# Plan 1 plan-session probe — provari checker build and JSONL contract

Date: 2026-10-29. Read-only probe; no product code changed.

## Goal

Confirm, before the execution session, that the pinned provari oracle builds
in this container and that its JSONL proof reader behaves as read from
`src/check.rs` (header with `root`, one line per attacker position, attacker =
side to move at the root).

## Oracle

- Repository: `https://github.com/aiorla/provari`
- Commit: `48686bc6c786d6ea70568f0b2ad4002cf0520206` (`v0.1.0`)
- Build: `cargo build --release` in `/tmp/provari`, no `bundled` feature
- Result: success in ~25 s; `provari 0.1.0` (2.9 MB binary)

## Hand-written proof

`/tmp/proof.jsonl`:

```json
{"root":"k7/8/1Q6/8/8/8/8/7K w - - 0 1"}
{"fen":"k7/8/1Q6/8/8/8/8/7K w - - 0 1","move":"b6b7"}
```

Command:

```sh
/tmp/provari/target/release/provari check /tmp/proof.jsonl
```

Output (exit 0):

```json
{"highest_counter":0,"moves":null,"proof":"/tmp/proof.jsonl","valid":true,"won_positions":1}
```

## Interpretation

The reader path, root-FEN handling, and root-`Win` attacker assumption are
confirmed. This probe did **not** exercise the exporter, root-`Loss` handling,
or a multi-line tree; those remain plan1's execution work. No GPL source or
binary is committed.