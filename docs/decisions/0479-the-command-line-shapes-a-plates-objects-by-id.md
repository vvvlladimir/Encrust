# 0479. The command line shapes a plate's objects by id

- **Status:** Proposed
- **Date:** 2026-11-25

## Context

The command line shaped one object per file given: it repaired, placed, hollowed and
supported the first object an input became and refused a cut model's halves, and it refused
every shaping flag on a saved project, "change it in the window". A chat client cut a model
and could not hollow or support the halves in the same job, and had to start a job per half.
Every object of a plate already has an id kept across saves (ADR 0466), and every flag is
already an edit of one object (ADR 0472); nothing reported the ids.

## Decision

- **Flags shape every object of the plate, or those `--only` names.** Repair, placement,
  relief, modelling, hollow, drains, plugs, supports and copies each run on every such
  object, a piece of a cut included; a plug is never one. A flag naming one point names one
  model, so with several shaped it asks for `--only`; an id no object has is refused.
- **A project takes the same flags,** made as edits of its plate before it is staged, and
  `--project-out` writes what they leave; a recipe, Check's answers, `--combine` and the
  island flags stay refused on it, since they prepare models as they come in.
- **Ids are said.** `inspect` of a project lists its objects; every report lists a model per
  object, not per file, with its `id`, a piece named `input #id`; a project's report says the
  cavity and supports it holds; `render --only` draws those objects alone.
- **The server follows.** A tool's step may name `objects`; a revision with such steps runs
  as the model with the steps for every object, then a run per group naming the same objects,
  on the plate the run before wrote, and keeps the last plate. `set_machine` moves a job to
  another printer and resin; a project's job is shaped as a model's.

## Consequences

A cut, its halves hollowed and supported, and one half changed alone are one job and one
command each. Reports change shape: a model given once and cut is two entries, and a script
reading `models[0]` of a cut model reads its first half. A revision with steps naming objects
costs a run per group and cannot go to the window as edits, so the window shows its plate. A
recipe still takes a model whole. Reopen if a recipe should run on a plate's pieces.

## Alternatives considered

### Several models in one job of the server

The server's job could hold several models — but every tool would name one, and the window
link edits one object; a job of one model whose plate holds its pieces needed neither.

### Groups of flags in one command, `--for 3 --hollow 2 --for 4 --supports heavy`

One run for all — but a parser of flags reads no groups, and a chain of runs through
`--project-out`, or one `--edits` file, says the same plainly.

### The option that won, and what it costs

A report per object breaks a script that read one entry per file, a run per group of objects
on the server, and point flags that need `--only` once several objects are shaped.
