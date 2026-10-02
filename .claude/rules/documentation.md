# Documentation rules

These bind every edit, including edits to documentation itself.

| What | Where |
|---|---|
| Why an approach, library or structure was chosen | `docs/decisions/NNNN-<slug>.md` |
| How an algorithm works, the maths, a data layout | `docs/design/<topic>.md` |
| What lives in which crate, and the dependency graph | `docs/architecture.md` |
| An external format and how we read it | `docs/formats/<format>.md` |
| A CLI flag and a worked example | `docs/cli.md` |
| A rule to follow from now on | `.claude/rules/<topic>.md` |
| Commands and status | `AGENTS.md` |

Code holds none of it, at most a one-line pointer. One fact lives in one file; every other
mention is a link to it.

`CLAUDE.md` and `.claude/rules/` load on every request, so lines there cost tokens forever —
spend them like that, and push anything a reader needs only sometimes down into `docs/`.

- **No growth without cause.** Prefer editing an existing file to adding one, and deleting a
  paragraph to appending one. Add no doc, rule, module, trait or abstraction for a case that
  does not exist yet.
- **Documentation is argument, not narration.** No benchmark diaries, no step-by-step
  retellings of what a step did, no restating the crate map in prose.
- When a doc stops matching the code, fix or delete it in the same commit that broke it.

An ADR is one decision per file, `NNNN-<slug>.md`, numbered from `0001` with no gaps, copied
from `0000-template.md` with every heading kept, well under a page. `docs/decisions/README.md`
carries the rest; `.claude/rules/workflow.md` says when one may still be edited.
