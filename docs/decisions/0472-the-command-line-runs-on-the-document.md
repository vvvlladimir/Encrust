# 0472. The command line runs on the document

- **Status:** Proposed
- **Date:** 2026-11-20

## Context

ADR 0466 makes every change to a plate an `Edit` carried out by `core_document::apply`, and
ADR 0471 makes the window the reference every front end matches. The window and `encrust-mcp`
run on the document; the command line still takes a model through its own pipeline
(`stage.rs`'s `put_down`, `hollowing.rs`, `supports.rs`, `stage/room.rs`) and its own recipe
executor (`recipe/{steps,plate,piece,drain}.rs`). Each fix of round 4 then had to be made twice,
and the command line still differs where the window is the reference: a lift planned before
drains, orient on the bare model, arrange before supports over the whole plate, points in the
imported frame, a plug gap typed raw. `--edits` and `--project-out` already run `apply` on a
project, so the command line has both paths.

## Decision

The command line's model pipeline is the document's (ADR 0466's phase 5). A run stages its
models into a `Document`, folds each shaping flag to an `Edit` in the window's order, carries
out each by `apply`, and slices the project it leaves as a project input is sliced today. A
recipe's plate steps are `edits_of` over the step's settings, the flags folded in, then
`apply`; `check`, `save`, `gate` and `pause` stay the executor's. The machine, the resin and
the slice stay the front end's: what an edit needs of them enters as a final number. The old
pipeline is deleted once every flag runs this way. The flag-by-flag mapping and the cards are
`docs/design/cli-on-document.md`.

## Consequences

One model gives the same plate from the window, the link and the command line, and a fix to a
step lands once, in `core-document`. `--edits` and the flags are interchangeable, which a test
holds. What no edit carried yet (keep-clear points, keep-solid runs, holes into the raft, merges,
repair's seam and shell limits) gets an edit, so MCP reaches it too; none is refused.
The JSON report is filled from the
objects and the edits' reports, so its numbers follow the window's. Reopen if a flag's
behaviour turns out to be one the window should take instead.

## Alternatives considered

### Keep the pipeline and patch each difference

Seven known differences, each a patch to a second copy of a step the document already has.
It keeps two ways to grow supports and drill holes, which is what ADR 0466 removes, and every
later fix is made twice.

### One executor, the flags folded into a recipe

The flags already fold into a recipe for `--recipe-out` (`recipe/folded.rs`), so a run with
flags could be a recipe run. Rejected as the only path: modelling flags (cut, split, label,
combine, relief, mends) have no recipe step, and a plain run would gain the recipe's report.
The fold to edits uses the same order, so the two stay one.

### The document, and what it costs

Eight cards over the command line's largest files, while the report and the batch keep their
shape. A run of flags shaping nothing must write the same bytes, which R6d1 proves before any
step moves.
