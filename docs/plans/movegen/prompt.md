When we implemented the basics (`plans/basics/report.md`) we integrated the `atomic-movegen` library.

What would be possible improvements to the library that would help us for this project?

Things that come to mind (not verified):
- `atomic_movegen::attacks::init()` must be called once before any move generation results bad performance?
- Should it track rule50?

Are there other possible improvements?

Write your findings to `plans/atomic_movegen_feedback/research.md`.

---

Kings touching in atomic chess is indeed allowed by the rules. Please re-think about section 2.6.

---

Based on the `plans/atomic_movegen_feedback/research.md` write feedback for the maintainer of the `atomic-movegen` library.
Write your feedback to `plans/atomic_movegen_feedback/feedback.md`.

---

The maintainer of the `atomic-movegen` crate integrated our feedback (`plans/movegen/feedback.md`) into their library.
Update to the most recent version and write a report to `plans/movegen/report_update_2.0.0`.


