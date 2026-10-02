# 0063. Price a field before filling it, and coarsen the lattice until it fits

- **Status:** Accepted, pricing superseded by 0085
- **Date:** 2026-09-22

## Context

Hollowing builds a narrow-band field around an isosurface of the model and marches a mesh
back out of it. Both costs follow the surface's area over the square of the lattice
spacing, and the spacing came from one number: `precision`, stated in millimetres and
clamped only by a wall four voxels wide and a floor of 0.05 mm.

Nothing bounded what that could cost. On a 406 000-triangle model 126 mm across, at a 2 mm
wall and precision 1.0, the run took 186 s and 2.47 GB; at a 0.8 mm wall with grid infill
at precision 0.7 it took 1.77 GB. The window holds the model, its hierarchy, the plate,
the preview stack and a GPU buffer beside that, so a run of this size is what was killing
the application rather than merely slowing it.

The spacing being absolute is what makes it unbounded: the same precision on a model twice
as long asks for four times the field, which is a setting that behaves differently on
every model a user opens. Stating the same handle relative to the model instead bounds
the grid a quality setting asks for by construction.

## Decision

Two bounds, and a price checked before anything is spent.

`lattice_mm` takes the model's longest side and never returns a spacing finer than
`size / 1500`, so precision is a quality setting over a model's own resolution rather than
a way to ask for an unbounded field. A wall is given three voxels rather than four, since
the wall's accuracy is half a voxel either way and the fourth cost a quarter of the field
for nothing a printer resolves.

`FieldSettings` carries a `budget_bytes`, 1.5 GB by default. `build` works out every tile
the band can reach before it fills a single one — that count is already computed to drive
the fill — and prices it at `TILE_COST_BYTES`, the tile's own values plus the triangles
marching cubes will cut out of it. Over budget, it returns `VolumeError::TooFine` naming
the spacing that would have fit, which is the current one scaled by the root of how far
over it is.

`hollow` acts on that rather than passing it on: it retries on a lattice a twentieth
coarser than the one the error named, up to three times, and reports the spacing it
settled on and whether it had to coarsen. The CLI prints both; the window says so under
the Hollow tool.

`FieldSettings` also carries a `clip`, so a field needed at one end of a model is not paid
for over all of it — the bottom-through floor builds the model's own field in the slab the
floor stands in and nowhere else.

## Consequences

A hollowing run cannot take the machine out of memory: the worst case is a cavity coarser
than was asked for, said out loud. On the model above, precision 1.0 went from 186 s and
2.47 GB to 14 s and 596 MB, and the thin-walled infill case from 1.4 GB to 731 MB.

Precision no longer means one fixed spacing. The same setting on a 30 mm model and a
300 mm one gives different lattices on purpose, and a user comparing two models will see
that. The cavity of a large model is visibly coarser than it was; it is inside the wall,
so nothing of the printed outside changes, but a mould made with External mode is the
cavity's own surface and does get blunter.

`TILE_COST_BYTES` carries an estimate of what a tile's triangles cost. It is a constant
measured on real models, not a derivation; if extraction changes what a tile produces, the
budget over- or under-reads until it is re-measured. The signal to reopen this is a run
that is refused while the machine is plainly not busy, or one that fits the budget and
still swaps.

A clipped field has to be told that a column cut short is still solid below the cut, which
is why `build` adds a sentinel tile past each end of a clipped column before working out
the solid runs. Forgetting that reads a cut column as empty space, which is exactly the
bug the first cut of this change had.

## Alternatives considered

### Refuse the run and make the user coarsen it

A clear error and no surprises about what was built. Rejected because the user has no way
to know which number to move or by how much — the answer is a function of the model's area
— and because every other slicer simply produces a coarser cavity.

### Budget by measuring memory as the fill runs

Watch the allocation and stop when it passes a ceiling. Rejected because half a field is
not a cavity: the run still has to start again from a coarser lattice, and now it has paid
for the abandoned half as well.

### The decision above, and what it costs

Pricing before filling means pricing an estimate. The tile count is exact, but what each
tile's triangles cost is not, so the budget is honoured to within the accuracy of one
constant. A model whose surface is mostly flat produces far fewer triangles than the
estimate and is coarsened when it did not need to be; one that is all detail may exceed
the budget it was cleared for. The alternative — pricing exactly — means building the
thing being priced.
