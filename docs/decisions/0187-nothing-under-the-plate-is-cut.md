# 0187. Nothing under the plate is cut

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

A stack started at the lowest point of the mesh, wherever it stood. A model is meant to stand
on the plate, but nothing made it: a hollow whose cavity was wrong put a strut 0.3 mm under
the floor, and the window wrote a file whose first layers stood at negative heights. A
printer told to expose a layer under its own plate drives the plate into the vat floor.

## Decision

`core-slicer` never plans a layer under the plate, which is `z = 0` in plate millimetres.
A uniform or adaptive stack starts at the mesh's bottom or the plate, whichever is higher,
so what stands under the plate is not cut. A mesh wholly under it is
`SliceError::UnderThePlate`. Every front end slices through these three entry points, so the
rule holds for the window, the browser and the command line alike.

## Consequences

No file this workspace writes has a layer at a negative height, whatever the mesh or the
placement. A model reaching under the plate loses that part without being moved. The run
logs a warning. That is the safe answer, but not a silent one: lifting it instead would
change the height of every layer above.

The command line slices a model where its file put it unless told `--center`. A model drawn
around its own origin used to be cut whole at negative heights and is now cut from the
plate up.

## Alternatives considered

### Lift the model onto the plate

It keeps the whole model, but it moves every layer, and the window already stands a model on
the plate when it imports it. A model still under the plate after that has been put there,
or is broken, and neither is fixed by moving it.

### Refuse any mesh that reaches under the plate

Safest, but a supported part sits a hair under zero after a rounding, and refusing it would
fail runs for nothing.

### The option that won, and what it costs

Part of a model can go unprinted with only a log line to say so. The window has to show it,
and so far only marks what stands outside the volume.
