# Agent Instructions (Read First)

## Non-Negotiable Requirements

- Always load and follow the latest user instructions, including any updates to `AGENTS.md` and `ROADMAP.md`.
- Create `ROADMAP.md` based on the provided input document.
- Continuously track progress by updating `ROADMAP.md` until all tasks in `ROADMAP.md` are completed.
- User might add new works under `User Work`, please finish them first and update the status.
- Do not proactively add arbitrary items under `User Work`, only add in `Agent Work`
- If `User Work` has no pending items, promote the next `Future Work` milestone into `Agent Work` as verifiable checklist items, then implement them in order.
- Do not ask the user any questions. Keep going until `ROADMAP.md` is fully completed.
- Work on the `main` branch. You may create git commits whenever necessary.
- After each git commit, run `git push` to `origin/main`.

## Working Principles

- Keep scope tight: implement only what is explicitly required for the current `ROADMAP.md` scope.
- Prefer small, reviewable patches.
- Do refactors when needed to keep the project maintainable.
- Avoid over-splitting work: when feasible, complete more scope in a single turn.
- Provide clear verification steps (commands + expected outcomes).
- Do not claim to have executed commands unless the execution is visible in the tool logs.

## Language Rules

- Discussion, reasoning, and summaries: Simplified Chinese.
- All code, identifiers, comments, and Markdown documents: English only.
