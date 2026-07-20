# Plan 3: Integerize `epsilon_ceil`

## Start

Read `docs/plans/speed/analysis.md`.  Locate `epsilon_ceil` in
`src/search/dfpn/core.rs` and the existing unit tests in the same file.

## Goal

Replace the `f64` ceiling computation in the DF-PN threshold loop with an exact
integer implementation.

## Background

`epsilon_ceil` is called every time the algorithm computes the child threshold:

```rust
pub(super) fn epsilon_ceil(&self, x: u64) -> u64 {
    if x >= INF {
        return INF;
    }
    let scaled = (x as f64 * (1.0 + self.epsilon)).ceil() as u64;
    scaled.max(x.saturating_add(1)).min(INF)
}
```

The conversion to `f64`, multiplication and `ceil` are tiny but repeated millions
of times per search.  Because `epsilon` is fixed during a search, we can
precompute it as a rational `num/den` and compute the ceiling with integer
arithmetic.

<ref_snippet file="/workspace/atomic_solver/src/search/dfpn/core.rs" lines="270-276" />

## Implementation tasks

1. Store `epsilon` as a pair of `u64` integers in `Search`: `epsilon_num` and
   `epsilon_den` such that `1 + epsilon = num/den`.  For example, `epsilon =
   0.25` becomes `num = 5`, `den = 4`.  `epsilon = 0.0` becomes `num = den`.
2. Precompute this pair when `set_epsilon` is called.
3. Rewrite `epsilon_ceil` as:
   ```rust
   pub(super) fn epsilon_ceil(&self, x: u64) -> u64 {
       if x >= INF {
           return INF;
       }
       let scaled = (x * self.epsilon_num + self.epsilon_den - 1) / self.epsilon_den;
       scaled.max(x.saturating_add(1)).min(INF)
   }
   ```
   Be careful to avoid overflow in `x * num`; use `u128` for the intermediate
   product or check `x > u64::MAX / num`.
4. Update the unit tests in `core.rs` to exercise the integer implementation with
   the same cases (`0.0`, `0.25`, `0.5`, `1.0`) and a few large values.

## File changes

- `src/search/dfpn/mod.rs` (`set_epsilon` field changes)
- `src/search/dfpn/core.rs` (`epsilon_ceil` and its tests)

## Risks

- Overflow in the integer multiplication if not widened to `u128`.
- `epsilon` is constrained to `[0.0, 1.0]`; `num/den` should stay within that
  range.
- The `f64` conversion currently rounds up; the integer formula must produce
  the same ceiling for every input.

## Verification

```text
$ cargo fmt
$ cargo clippy --all-targets
$ cargo test --all-targets
```

The existing `epsilon_ceil_*` tests and all solver tests must pass with
identical outcomes and PVs.

## Final task

Write `docs/plans/speed/report3.md` noting whether the change is measurable and
confirming the integer and floating-point versions are identical for the
supported `epsilon` values.
