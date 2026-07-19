Review the correctness of this implementation.
Note that there is research for the used techniquest in these files:
* docs/plans/dfpn/research_epsilon.md
* docs/plans/dfpn/research_ghi.md
* docs/plans/dfpn/research_parallel.md

Write a report to docs/plans/review/review1.md

---

Read the review docs/plans/review/review1.md and create implementation plans for the noted issues.
Write the plans to docs/plans/review/plan<x>.md where <x> goes from 1 to 8.
Each plan should start with reading the report of the last plan and have a final task which creates an implementation report docs/plans/review/report<x>.md where <x> goes from 1 to 8.
 
---

Review the correctness of this implementation.

Note that there is research for the used techniques in these files:
* docs/plans/dfpn/research_epsilon.md
* docs/plans/dfpn/research_ghi.md
* docs/plans/dfpn/research_parallel.md

Also note past reviews and the implementation reports in docs/plans/review/ .

Write a new review to docs/plans/review/review5.md

---

Read the review docs/plans/review/review2.md and create implementation plans for the noted recommendations.
Write the plans to docs/plans/review/plan<x>.md where <x> goes from 9 to 14.
Each plan should start with reading the report of the last plan and have a final task which creates an implementation report docs/plans/review/report<x>.md where <x> goes from 9 to 14.


---

Please evaluate the following feedback for docs/plans/review/review4.md. Treat it as a brainstorming session. What would be the best course of action from here?

# User feedback

## Feedback and questions

**2.1 GHI implementation correctness**
I.e. we need a test case to verify edge cases?

**2.2 1000 ply cap**
Can we treat 1000+ ply lines as lost for white?
Reasoning: The end goal is to prove that white wins in atomic chess. My assumption is that this true and it is a forced win in under 1000 plies.

**2.3 remaining_depth**
Could we simplify this by only having two outcomes? Win and not win.
A draw could be treated as a win for black, since i strongly suspect white has a forced win anyway.

**2.4 dead code**
Should be removed.

**2.5 Outcomes**
Same quesiton as in 2.3: Maybe we should only have win and not-win as outcomes for white?
I.e. black wins and draws are both treated as wins for black.

**2.6 Draw PV validation**
Same as 2.3/2.5? Could a two-outcome approach simplify things?

## Other findings

**Size of dfpn.rs**
src/search/dfpn.rs has a file size of 54kb
It is to big and needs to be split it into smaller files.
We should also add a convention in the AGENTS.md which mandates a max size of around 10kb per file, violations need documented justification.

---

Please describe in details what kind of position(s) we need to have a robust GHI test.
