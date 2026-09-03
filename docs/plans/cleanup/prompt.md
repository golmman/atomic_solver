Review the code for inconsistencies, DRY and YAGNI violations, general housekeeping opportunities.
The correctness must not be compromised though.
Write an implementation plan with proposed fixes to `docs/plans/cleanup/plan2.md` .

---

Last session we finished with `docs/plans/nn/report8.md` which proved the PoC a failure.

I don't want that the code is cluttered with features we don't need.
So I want the repository to revert back to the pre nn-era without losing the intermediate improvements to testability or other general improvements (to the AGENTS.md, new test positions etc.).

Here is my plan:
* go back to `3ff0941` which is right before the first nn-commit
* put all the following commits into a dedicated `nn` branch
* cherry-pick non-nn improvements:
  * 17eb7d2
  * 2cec9fe
  * 859b296
  * 9020836
  * 6009f4d - tricky one, bump to atomic-movegen v2.1.0 is appreciated, changes to nn trainer_init are not needed
  * 0cad506
  * 8ab512e
  * 4c5c111
  * 64e4ea8
  * 3fcbd48
  * 88e6851

Note that i simply went through the commit-messages to filter this list of commits, so treat this just a suggestion.

Please come up with a sound plan and write it to `docs/plans/cleanup/plan3.md`. Ask questions where needed.
