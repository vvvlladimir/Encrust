# 0303. Combine models exactly, and through the field where one is not a solid

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

A user wants to add one model to another, take one out of another, or keep only what two
share: a base fused to a figure, a socket cut by a peg, a part cropped by a box. The
slicer already unions overlapping models by the fill rule (ADR 0071), but only while
slicing; the plate keeps two models, an export keeps both shells, and nothing subtracts.
`core-csg` cuts closed meshes exactly (ADR 0302), and `core-volume` already combines
fields, which read any mesh with a winding number, open scans included.

## Decision

`core_volume::combine` takes models placed on the plate and a `Boolean` — union,
difference of the first less all the others, or intersection — and answers one closed
mesh. It cuts exactly through `core-csg`, each model's shells unioned first — the engine
reads crossing shells silently and counts their overlap twice — the operands folded down a
tree in parallel and a difference's subtrahends unioned first. Where any model is not a solid the engine reads,
every model is sampled into a field on the lattice precision 0.5 gives their area,
coarsened to the budget, combined there and extracted; `Combined::voxel_mm` says so.
`combine_exact` never samples and refuses with `NotSolid`, for a caller that must not
remesh the model, as a label must not (ADR 0304).

In the window the tool sits in the Relief panel, under Combine models. It takes the models
picked on the plate, in the order they were picked, and runs off the drawing thread. The
result replaces them, named after the first, through the same placement a cut uses; their
supports, cavity, holes and texture are not carried over. The model itself is combined,
never its shell. Undo puts the originals back, as for any edit (ADR 0086).

## Consequences

A union, a socket or a crop is one model that exports as one closed shell. Sharp edges stay
sharp wherever every model is closed; a torn scan still combines, with its edges rounded to
the lattice, and the stage notice says which lattice. Hollow, supports and holes are built
again on the result. Reopen if users want the supports or holes carried over, or if the
field path turns out to be taken on most real scans.

## Alternatives considered

### Combine only through the field

What the plan first offered: one path, robust on any mesh. It remeshes every model at the
lattice, so a sharp CAD part comes back faceted and a million faces heavier.

### Exact only, refusing a torn model

Simpler, and never rounds. Scans are a large share of what is printed, and refusing them
leaves a user nothing to do but repair first.

### The option that won, and what it costs

Two paths to keep and test, and a result that silently changes character with its input;
the notice is the only thing telling a user their edges were rounded. Supports, cavity and
holes are lost on every combination.
