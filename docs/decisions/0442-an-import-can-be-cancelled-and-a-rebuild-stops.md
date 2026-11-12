# 0442. Let an import be cancelled, and stop a sliced file's rebuild

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

ADR 0038 left imports uncancellable: a mesh read is one call into a loader that has no
place to stop, and a wrong file only cost waiting. Opening a sliced file as a model
(ADR 0294) is different work: every layer is spread into a field, then a surface is read
out of it, and both are loops the thread controls. A full 16K plate takes seconds, a
`.sl1` a minute, with nothing on screen to say how far it has got or to stop it.

## Decision

An import shows a job card at the stage's foot (ADR 0263) with a Cancel. Cancel drops
every import under way. Each `ImportJob` holds an `ImportWatch`; dropping the job sets its
stop flag. A sliced file's rebuild asks the flag before each layer and passes it to
`extract` as its `Cancel`, so it stops within a layer or a layer of tiles. A mesh read does
not ask and runs to its end, and its outcome is thrown away with the handle.

The watch also counts the rebuild: layers built as the first two fifths of the bar, the
surface's layers of tiles as the rest. A mesh import counts nothing and its bar runs
without a share.

## Consequences

A wrong or slow sliced file can be stopped, and the card says how far the rebuild has got.
A cancelled mesh read still costs its thread and memory until it ends, as before. If a
large mesh that cannot be stopped becomes a complaint, the loaders take the flag next.

## Alternatives considered

### Stop mesh reads too

Every loader would take a flag and ask it between chunks. That changes each
`core-mesh-io` reader for a case nobody has reported; a mesh read is seconds, not a minute.

### The option that won, and what it costs

Cancel on a mesh import only hides it: the thread still runs and a second open of the same
file meanwhile reads it twice.
