# 0484. A step for some objects keeps the others' steps

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

By ADR 0479 a tool names some of a job's objects by `objects`. A revision kept one hollow, one set
of drains and one modelling, so `set_hollow` for object 2 after object 1 made object 1 solid
again and said nothing. Supports alone kept the others, in `more_supports`.

## Decision

Modelling, hollow, drains and supports follow one rule (`recipe/scoped.rs`). A step asked for
some objects stands over what was asked before for those objects alone, the others keeping
theirs; a step for every object, or none, stands in for all before it. A revision keeps the first
asked as `modelling`, `hollow`, `drains`, `supports` and those asked later for other objects as
`more_modelling`, `more_hollow`, `more_drains`, `more_supports`, staged as one run each by
`--only`. Turning a model clears hand-given drain points in every one of them.

## Consequences

Shaping pieces one by one keeps what each was given. Making one piece alone hollow takes two
calls: none, then that piece. The revisions a job kept before read as they were. Reopen if a
step should merge with an earlier one for the same objects rather than replace it.

## Alternatives considered

### Each step a list

Rejected: the job files kept before would no longer read, and every place that reads one step
would change for a case the first entry covers.

### Refuse a step for some objects where another is kept

Rejected: it tells the client the loss but still makes it work round it.

### The option that won, and what it costs

Four fields in pairs, and a client that wants one piece alone asks twice.
