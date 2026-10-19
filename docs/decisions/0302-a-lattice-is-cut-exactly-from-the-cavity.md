# 0302. Cut the lattice exactly from the cavity's mesh, its cells joined by crossing holes

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

ADR 0060 meshes the lattice as boxes the non-zero winding rule unions, and ADR 0189 opens
every cell with a 1.5 mm slot over the floor and under the ceiling. The infill resin
printers are expected to offer is another shape: thick walls between hexagonal or square
cells, joined by round crossing holes row above row, and a scaffold whose struts all lean.
That is subtraction — a hole through a wall, a cell clipped by a curved shell — and boxes
can only add. The clip was a sampled walk, `meet` and a bond, all approximate; a scaffold on
the axes had level struts that start in mid-air; and the resin saved was the density times
the cavity.

Cutting the lattice out of the cavity's field, `max(cavity, -solid)`, was tried first. It
is exact in what it keeps, but it is the cavity's lattice: at precision 0.5 the walls came
out crumpled at 0.2 mm, the faces ran to millions, and a 140 mm block took 2–3 s for the
lattice alone. `core-csg` now cuts closed meshes exactly (ADR 0303).

## Decision

Each pattern is written down as closed meshes — prisms, cylinders, square bars — and as a
signed distance. A run cuts the meshes from the cavity's mesh exactly, and only where that
mesh is not a solid the engine reads does it cut the distance from the field, priced
against the budget and refused with `TooFine` past it.

The shapes come in groups whose shells never cross. The cavity stays whole as the void, and
the lattice is appended as solid, wound outward: a hive's or grid's walls are the cavity
less its cells and then less each group of holes in turn, one solid with real openings; a
scaffold is the cavity ∩ each group of struts, appended to overlap at the nodes for the fill
rule to union. Holes reach into the cells past their walls, so no face of a hole lies on a
wall's.

A shape whose box meets no tile of the cavity's field is left out, and the cavity is halved
by the engine itself, intersection and difference with one box, until there are two pieces a
thread; the pieces are carved in parallel and appended, meeting face to face on each cut.

`InfillSettings` states sizes, not a density, for the patterns with walls: `cell_mm`,
`wall_mm`, and crossing holes `hole_mm` across — under a hexagon's side or a square's — the
first row `first_hole_mm` over the cavity's floor and then every `hole_spacing_mm`, further
apart than a hole. Scaffold keeps `cell_mm` and `density`, and stands its cubic lattice on
a corner so every strut leans 35° off level; its density is inverted exactly from
`3x² - 2x³`. A project saved with the old fields does not open.

## Consequences

Walls are flat, holes round and open in the viewport as they print, and the cavity less the
walls is the resin saved. One drain hole empties every cell. On a 140×70×80 mm block at
precision 0.5, the lattice cut whole cost 1.4–5.0 s against 2.4–3.2 s from the field and a
third to a thirteenth of its faces; unioning a scaffold's groups first took up to 17 s on
the same block, which is why they are not. Culled and in pieces, a hive on a 100 mm ball
drops from 5.9 s to 2.4 s and a grid from 2.3 s to 1.3 s; a scaffold, already three cuts in
parallel, gains nothing there and a quarter on a real bust. See `docs/design/hollowing.md`
for the tables. The hive is still the dear one: its holes are cut through the walls one
group after another. Each cut between pieces leaves a pair of faces inside the walls, which
no slice reads as a boundary. The exact cut's triangulation varies from run to run
(ADR 0303), so a shell is reproducible in its surface and not in its triangle list.

Reopen if the fallback turns out to be taken on real models, or if a sliver cell at the rim,
lower than the first row, proves to be a pocket often enough to matter.

## Alternatives considered

### Cut the lattice from the field alone

What this record first said. Robust on any cavity and needing no new crate, but rough at the
precision a user picks, heavy in faces, and priced in the cavity's volume.

### Keep the boxes and add the holes wound inward

A drain hole is already a body wound inward (ADR 0188). It cancels a single wall, but not
where boxes overlap at a junction or a bond, and it does nothing for thick hexagon walls or
for the clip.

### Evaluate the lattice per layer at slicing time

Memory is one layer and any pattern is affordable, but it is a second slicing path through
the preview, the viewport and the drainage scan, and the model stops being one mesh
(ADR 0059).

### Halve the cavity with a plane cut

`core-geometry::cut` is linear and cheaper than a boolean, but at the seams surface nets
leaves where two sheets of a cavity touch it left loops open or an edge walked three times,
on two of three real models tried.

### The option that won, and what it costs

Every pattern is written twice, as meshes and as a distance, and the two are held to each
other by a test. A scaffold is several overlapping shells, so its mesh is not one manifold.
A hive costs several times what appending its holes to a void of cells did, which drew a
membrane across every hole. A cavity mesh the engine cannot read gets the rough field
instead, silently.
