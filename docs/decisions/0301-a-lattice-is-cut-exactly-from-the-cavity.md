# 0301. Cut the lattice exactly from the cavity's mesh, its cells joined by crossing holes

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
lattice alone. `core-csg` now cuts closed meshes exactly (ADR 0302).

## Decision

Each pattern is written down as closed meshes — prisms, cylinders, square bars — and as a
signed distance. A run cuts the meshes from the cavity's mesh exactly, and only where that
mesh is not a solid the engine reads does it cut the distance from the field, priced
against the budget and refused with `TooFine` past it.

Nothing is unioned: the shapes come in groups whose shells never cross, each group is one
boolean against the cavity, and the pieces are appended to overlap as they will, for the
fill rule to union. A hive or grid is the cavity ∩ its cells plus its holes — whole where a
hole stands clear inside the cavity — wound inward; a scaffold is the cavity ∩ its struts,
wound outward over the whole cavity. Holes span their walls face to face and struts stop at
nodes, so pieces never overlap and the volumes add up.

`InfillSettings` states sizes, not a density, for the patterns with walls: `cell_mm`,
`wall_mm`, and crossing holes `hole_mm` across — under a hexagon's side or a square's — the
first row `first_hole_mm` over the cavity's floor and then every `hole_spacing_mm`, further
apart than a hole. Scaffold keeps `cell_mm` and `density`, and stands its cubic lattice on
a corner so every strut leans 35° off level; its density is inverted exactly from
`3x² - 2x³`. A project saved with the old fields does not open.

## Consequences

Walls are flat, holes round, and the void's volume is the resin saved. One drain hole
empties every cell. On a 140×70×80 mm block at precision 0.5, the lattice costs 1.1–1.8 s
against 2.4–3.2 s from the field and a fifth to a thirteenth of its faces; see
`docs/design/hollowing.md` for the table. Unioning the groups first took up to 17 s on the
same block, which is why nothing is unioned.

Overlapping shells cost contours — twice as many on a hive's busiest layer — and slicing the
plate takes as long as before. The exact cut's triangulation varies from run to run
(ADR 0302), so a shell is reproducible in its surface and not in its triangle list.

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

### The option that won, and what it costs

Every pattern is written twice, as meshes and as a distance, and the two are held to each
other by a test. The void is several overlapping shells, so its mesh is not one manifold
and more contours reach the slicer. A cavity mesh the engine cannot read gets the rough
field instead, silently.
