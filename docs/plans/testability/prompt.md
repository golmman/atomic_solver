My goal is to prevent regressions by improving the test suite and increasing general testability.

## User Idea

### Step 1 - test suite improvements
* improve existing tests
* review skipped/ignored tests
* remove stale and unecessary tests
  * keep tests that are ignored for a good reason
* add new tests where sensible
* add examples which can be used to debug / analyze the application

### Step 2 - testability improvements
Analyze where the code could be improved in such a way that testability is increased, but without compromising on correctness or performance.

## Agent Task

Create at least two separate implementation plans (`docs/plans/testability/plan1.md`, ...), one for each step, or even more if advised.


---

debug assertions?


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

---

I like your ideas. Don't it implement yet. Write an implementation plan to `docs/plans/testability/plan3.md`.
