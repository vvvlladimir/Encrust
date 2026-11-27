# 0458. The command line runs on the document and shapes a plate's objects by id

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

After ADR 0456 the window and the server run on the document, but the command line still took a
model through its own pipeline and recipe executor, so each fix was made twice and the window's
order — drains before the lift, orient on the placed model, arrange after supports — was not
kept. It shaped one object per file, refused a cut's halves and refused every shaping flag on a
project; a chat client that cut a model could not hollow the halves in the same job. A step
asked for one object replaced the same step asked for another.

## Decision

- **A run stages its models into a `Document`**, folds each shaping flag to an `Edit` in the
  window's order, carries each out by `apply` and slices the project it leaves. A recipe's plate
  steps are `edits_of` then `apply`. The mapping is `docs/design/cli-on-document.md`.
- **`--lift MM`** stands a model that high as the window's row does; `Edit::Orient` carries
  `lift_mm`, so every front end weighs a pose at the same height.
- **Flags shape every object, or those `--only` names**, a cut's pieces included, a plug never;
  a flag naming one point needs `--only` once several are shaped. A project takes the same
  flags as edits of its plate. `inspect` lists object ids, every report lists a model per object
  with its `id`, and `render --only` draws those alone.
- **Pieces are shaped after the cut**: the pose search, the turn and the lift run on each piece.
- **The server follows.** A tool's step may name `objects`. Modelling, hollow, drains and
  supports asked for some objects stand over what was asked for those alone, the others kept as
  `more_*`; each group is a run by `--only` on the plate the run before wrote.

## Consequences

A fix to a step lands once, in `core-document`; `--edits` and the flags are interchangeable,
which a test holds. A cut, its halves hollowed and one half changed alone are one job. Reports
change shape: a model cut is two entries, so a script reading `models[0]` reads its first half.
A run with steps naming objects costs a run per group and reaches the window as a plate, not
edits. Reopen if a recipe should run on a plate's pieces.

## Alternatives considered

### Keep the pipeline and patch each difference

Seven known differences, each a patch to a second copy of a step the document has.

### Several models in one server job

Every tool would name a model and the window link would be rebuilt; a job of one model whose
plate holds its pieces needs neither.

### The option that won, and what it costs

Eight cards over the command line's largest files, a report per object that breaks scripts
reading one per file, and a client that wants one piece alone hollow asks twice.
