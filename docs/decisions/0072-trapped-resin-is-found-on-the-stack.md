# 0072. Find trapped resin on the slice stack, in `core-supports`

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

A hollow model with no way out holds its cavity full of uncured resin. Finding those
pockets is the other half of step 8e, and it has to answer two questions: what the
algorithm reads, and which crate owns it.

Other slicers voxelise the model for this. We already cut the stack, and `core-supports` already
reads a layer onto a grid as runs (`Field`, ADR 0032) — so the pockets are the connected
components of the *air* between those runs, walked bottom-up. That is one pass over data
the run produces anyway.

The crate is the awkward part. A field is `core-volume`'s business, but `core-volume` sits
under `core-slicer` in the dependency graph and cannot see a `Layer` at all. `Field` lives
in `core-supports`, and the rule against duplicating a type means the scan goes where the
type is or the type moves down to a new shared crate.

## Decision

`core-supports` owns it: `TrapScan` reads layers in print order and answers with a
`Trapped` per pocket — what it holds, and a point inside its lowest layer for a hole. The
crate's responsibility is now stated as analysis over the slice stack, which is what both
island detection and this are.

The scan keeps one layer of air runs and the pockets still open above it, never the stack,
so it folds into a pass that is already streaming (ADR 0066, 0068): the CLI feeds it from
the same window walk that writes the file, and the window's own check cuts window by window
and keeps nothing.

A pocket is open — and so not reported — when it reaches the edge of the grid, when it is
alive on the first layer, which drains onto the plate, or when it is still alive at the top
of the stack. Below five cubic millimetres nothing is reported: that is a droplet, not a
failure.

## Consequences

The check costs a rasterisation of each layer onto the support grid, a tenth of a
millimetre a cell, and is bounded in memory by one layer however tall the stack.

It sees what the printer sees, so it is right about things a mesh-only check is not: an
infill cell that seals itself, a blocker that walled a cavity off, two models overlapping
on the plate. It also inherits the grid: a drainage channel narrower than a cell reads as
closed, and the pocket behind it is reported as trapped. That fails towards a hole nobody
needed, not towards a print full of resin.

`core-supports` now carries a second subject. The signal to split it is a third one, or a
second crate wanting `Field` — that is when `Field` goes down into `core-raster` and the
stack analysis becomes its own crate.

## Alternatives considered

### Flood-fill a voxel field in `core-volume`

What a voxelising slicer does, and it would sit beside hollowing where the rest of the
cavity work is.
Rejected because it builds a second representation of a model we have already sliced, at a
cost that follows the volume rather than the outline, to answer a question the stack
answers exactly.

### A new `core-drain` crate over `core-raster`

Honest boundaries: move `Field` down into `core-raster`, put the scan in a crate of its
own. Rejected for now as a crate and a type migration for one algorithm, with no second
caller to justify either.

### The option that won, and what it costs

`core-supports` is no longer only about supports, and its name says it is. Anyone looking
for the drainage check will look in `core-volume` first, because that is where hollowing
is, and find a pointer rather than the code.
