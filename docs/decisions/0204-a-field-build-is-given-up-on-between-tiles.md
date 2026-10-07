# 0204. A field build is given up on between tiles

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

A hollowing run is one call into `core_volume::hollow`, and on a real model it takes seconds:
a 30 mm sphere at precision 0.8 takes nine and a half. The command line reads its Ctrl-C flag
between stages and between windows of layers, so a stop pressed three seconds into the cavity
was answered when the cavity was finished — the run looked hung, and a second press was the
only way out. The window had the same gap, and said so in a comment: its Cancel button could
only land between models.

Nothing inside the build was watching anything. The work is a parallel walk over tiles in
`sweep::fill`, a parallel carry before it, and a layer-at-a-time merge in `extract`, all of
them made of pieces far smaller than a second.

## Decision

`core_volume::Cancel` carries what the caller is asked: `Cancel::never()` for a run nothing
stops, `Cancel::when(&predicate)` for one that answers a flag. `hollow`, `hollow_at_scale`,
`build` and `extract` take one. It is asked between the rings of the carrier, once a tile in
the sweep, between the layers of tiles in `extract`, and between the coarsenings a budget
forces; a build given up on answers `VolumeError::Cancelled`, and `extract` answers an empty
mesh, which its caller turns into that error.

The command line hands it the same `Stop` its Ctrl-C handler sets, and maps `Cancelled` to its
own `Cancelled` error, which is already exit code 130.

## Consequences

The first Ctrl-C ends a hollowing run inside it, on any model. The predicate is called once a
tile, so it must stay as cheap as an atomic load — the type carries nothing else to make that
hard. Every caller of these four functions names what it wants, which is also a reader's
answer to "can this be stopped?".

The infill lattice is not watched yet: a stop pressed while a hive is being built is answered
when the hive is finished. The window still passes `Cancel::never()`; wiring its Cancel button
through is a change to that button's behaviour and belongs with it.

## Alternatives considered

### A flag in `HollowSettings`

The settings are already handed down the whole path, so nothing would need a new parameter.
They are also `Clone`, `PartialEq` and `Debug`, they are serialised as part of a project, and
they are compared to decide whether a cavity has to be rebuilt. A predicate is none of those
things, and making the derives work around it would put a cancellation flag inside the data a
project records.

### The option that won, and what it costs

Four public signatures grew a parameter, and some twenty calls in tests and benchmarks now say
`Cancel::never()` to mean "as before". A borrowed predicate also gives `Cancel` a lifetime,
which anything holding one has to carry.
