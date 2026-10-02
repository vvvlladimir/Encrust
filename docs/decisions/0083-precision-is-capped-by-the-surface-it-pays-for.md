# 0083. Cap precision by the surface a field pays for, not by the model's longest side

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

ADR 0063 bounded `precision` so it could not ask for an unbounded field: no spacing finer
than the model's longest side over 1 500. That made precision a quality setting rather
than a way to run the machine out of memory, and it worked.

It is the wrong quantity. What a field costs is the surface's area over the square of the
spacing — that is the same formula the budget of ADR 0063 prices tiles with, and the same
one ADR 0082 reasons about. A 150 mm spire and a 150 mm block have the same longest side
and nothing alike in surface: under the side-based ceiling the spire was capped at 0.1 mm
when it could afford 0.05 mm, and a bulky model of the same height was let through at a
spacing it could not.

## Decision

`lattice_mm` takes the model's surface area instead of its longest side, and never returns
a spacing finer than `sqrt(area / MAX_LATTICE_SQUARES)` with `MAX_LATTICE_SQUARES` at
seven million.

Seven million is not a new judgement: it is the number that reproduces the old ceiling on
a ball the width of a Mars 4 plate, `π · 1500²`. A model with more surface than a ball of
its size — a block, a lattice, a scan full of detail — is now capped tighter than it was,
and a slender one looser.

`Mesh::surface_area` lands in `core-geometry` beside the mesh, because both `core-volume`
and the Hollow panel need it.

## Consequences

Precision means one thing on every model, and it means the right one: the same setting on
two models now costs about the same rather than looking the same. A spire hollows at the
spacing it asks for.

A bulky model is cut coarser than it used to be at the same precision. The cavity is
inside the wall so nothing printed changes, but a mould made with External mode is the
cavity's own surface and does get blunter — the same trade ADR 0063 already made, now
applied to the models it should have applied to.

Surface area costs a pass over the faces. It is a sum of cross products over a mesh that
has just been loaded and hierarchied, so it does not show.

The signal to reopen this: a model that is refused or coarsened while its area says it
should fit. That would mean the band's thickness, which the constant folds in silently,
has stopped being roughly two voxels.

## Alternatives considered

### Cap by volume

What the roadmap line asked for. Rejected because volume is not what a narrow-band field
costs — a solid block and a hollow shell of the same bounds cost the same field and have
wildly different volumes.

### Cap by the tile count the walk finds, and coarsen from that

Exact, and the budget of ADR 0063 already does it one step later. Rejected as the ceiling:
it only speaks after a walk over the faces, so `precision` would have no spacing to show
in the panel until a run had started.

### The decision above, and what it costs

Area is a proxy too. It ignores how the surface is folded: a model whose faces are packed
into a small volume — a sponge, a lattice already printed into the mesh — has its band
overlap itself, and costs less than its area says. Such a model is now coarsened for no
reason, and only the budget will ever tell us, because nothing measures the overlap.
