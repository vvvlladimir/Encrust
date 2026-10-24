# Documentation rules

| What | Where |
|---|---|
| Why an approach, library or structure was chosen | `docs/decisions/NNNN-<slug>.md` |
| How an algorithm works, the maths, a data layout | `docs/design/<topic>.md` |
| Which crate owns what, the dependency graph | `docs/architecture.md` |
| An external format and how we read it | `docs/formats/<format>.md` |
| A CLI flag and a worked example | `docs/cli.md` |
| What a user can and cannot do | `docs/features.md` |
| A rule to follow from now on | `.claude/rules/<topic>.md` |
| Commands and status | `AGENTS.md` |

Code holds none of it beyond a one-line pointer. One fact lives in one file; elsewhere, a link.

## Instruction files stay small

`CLAUDE.md`, `AGENTS.md` and `.claude/rules/` load on every request and cost tokens forever.

- Each file at most 40 lines; those without `paths:` together at most 250 (`wc -l`).
- A rule is the instruction in the fewest words that carry it: no rationale, no history, at
  most one example. The why belongs to an ADR.
- A rule is added only for a mistake that happened or one a review would miss, and a line
  added means a line tightened or removed. What is needed only sometimes goes to `docs/` or
  a rule scoped by `paths:`.

## Every document

- Edit over add, delete over append; nothing for a case that does not exist yet.
- Argument, not narration: no diaries, no retelling of a step, no restating the crate map.
- A doc that stops matching the code is fixed or deleted in the commit that broke it.
- A change a user can see updates `docs/features.md` in the same commit.
- An ADR is one decision, `NNNN-<slug>.md` numbered without gaps, from `0000-template.md`
  with every heading, under a page; the rest is `docs/decisions/README.md`.
