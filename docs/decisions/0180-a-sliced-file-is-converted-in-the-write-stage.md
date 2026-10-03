# 0180. A sliced file is converted in the write stage, never resampled

- **Status:** Accepted
- **Date:** 2026-10-03

## Context

Every container has a reader and a writer (ADR 0149), so a file sliced for one machine can
in principle be rewritten for another that reads a different container. A reader hands back
the masks, the layer heights and exposures and the panel size; it does not hand back a
printer profile, the lifts, the speeds or the waits, which are in the header in as many
shapes as there are containers.

## Decision

`core_pipeline::convert` and `convert_to` read every layer of an opened file and push it,
in groups encoded in parallel, into the writer the output format picks — through the same
dispatch `write_to` uses, now generic over what feeds the layers. The caller gives the
printer, whose panel must be the file's to the pixel, and a resin for the lifts, waits and
price. The layer height, normal and bottom exposure and bottom count are the file's; a layer
above the bottom block exposed differently from the header becomes an exposure band. A
stack of varying layer heights is refused, and the previews are written blank: no reader
hands its previews over yet.

## Consequences

`encrust convert` is a reader and a writer and no slicing. A panel of another size is an
error rather than a resample, so a converted file is the same masks or nothing. A file's
motion settings do not survive the trip; they come from the resin chosen for the new
machine, which is the right source when the machine changes and a loss when it does not.

## Alternatives considered

### In `core-engine`

`core-engine` is a plate in and a file out (ADR 0174); a file in and a file out has no plate.
`core-pipeline` already holds both the readers' dispatch and the writers', so the
conversion adds no dependency anywhere.

### Resample to the new panel

Every edge would move by up to half a pixel and the anti-aliasing would be resampled grey,
which is a different print presented as the same one. Slicing the model again is the honest
path to another panel.

### The option that won, and what it costs

A machine's motion settings are not carried, and a stack with varying heights waits for a
per-layer exposure in `PrintJob`.
