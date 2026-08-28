Review the code for inconsistencies, DRY and YAGNI violations, general housekeeping opportunities.
The correctness must not be compromised though.
Write an implementation plan with proposed fixes to `docs/plans/cleanup/plan2.md` .

---

On this machine host (asahi m1) running the test suite takes more than 25 minutes, even with release-flag (`cargo test --release`).

**My opinion**

In my oppinion `cargo test` should run no more than 60 seconds and perform unit tests and run simple performance tests, maybe some important integration tests.
If there are good reasons we could weaken the 60 secs requirements to `cargo test --release`.

Only extraordinary changes should require running the complete test suite so it shouldn't be the default. It should still be possible to run them via a simple additional flag or via a dedicated phony make target script.

Benchmarks, stress tests, performance tests, extended integrative tests should only be executed if justified.

**Your Task**

Is my opinion sound and justified? If you agree, what should be changed and how? What are the (non-obvious) trade-offs?
Please answer my questions and come up with new ideas.

