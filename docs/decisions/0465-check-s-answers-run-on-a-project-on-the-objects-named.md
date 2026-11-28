# 0465. Run Check's answers on a project, on the objects `--only` names

- **Status:** Accepted
- **Date:** 2026-11-27

## Context

ADR 0459 ran Check's answers — `--support-islands`, `--drill-cups`, `--hold-necks` — as a
recipe's last `check` step and refused them on a project, since a recipe prepares models as
they come in from their files. ADR 0464 then made a chat client's points name objects, and a
recipe naming objects is staged as runs on the plate the run before wrote: a project. So once a
client gave one drain point on a cut model, `fix_issues` was refused for the rest of the job,
and the client answered each island, cup and neck by hand, a run each. The step that answers
them, `Run::check`, already measures the whole plate and answers only the pieces it is given.

## Decision

- **The command line runs Check's answers on one project's plate**, after the flags' edits and
  before copies and the spread, as a recipe of that one `check` step. Every object of the plate
  is a piece, so an island is put down to the object it hangs from; the step runs on the objects
  `--only` names, every one without it. Several projects laid together and `--all-plates` still
  refuse them.
- **`fix_issues` takes `objects`**, as every other step does. Answers asked are a run of their
  own, last, on the plate the run before wrote, by `--only` where they name objects; a recipe
  whose steps name objects no longer refuses them.

## Consequences

A client answers a cut model's halves, or one of them, in one call, whatever points it gave
before. A run on a project with answers costs a measure of the whole plate per pass, as on
models. An answer's support stands to the object's support profile only where the flags name
one, else `auto`, which a plate supported to a named profile may notice. Reopen when the window's
Fix all on a selection and the command line's answers part ways.

## Alternatives considered

### Answer on the models' run only, and keep the server's refusal

No change to the command line — but a client that gave one point lost Fix all for the job.

### Measure the named objects alone

Cheaper per pass — but an island under one object that hangs from another would be put down
to the wrong one or none.

### The option that won, and what it costs

A second way into `Run`, built from a plate rather than from files, whose pieces carry an
import report read off the saved mesh; a step reading what an import found sees no repair.
