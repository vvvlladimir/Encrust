# Hollowing and infill

How a solid becomes a shell and what fills it. `core-volume`: `hollow.rs` holds the modes,
blockers and assembly, `infill.rs` and `infill/` the lattices. Both stand on `volume.md`.

Why: ADR 0058 (band on the isosurface), 0059 (cavity appended, not welded), 0063 (the
budget), 0083 (the precision ceiling), 0189 (a hole is what drains a cavity), 0260 (kept
fields), 0302 (the lattice is cut exactly from the cavity's mesh).

## What comes out

`hollow` returns the whole solid ready to slice, plus the volume the cavity saves: three
meshes appended in one buffer and nothing else.

| | Internal | External |
|---|---|---|
| outside | the model, byte for byte | the surface grown by `t` |
| void, wound inward | the surface `d = -t` | the model, wound inward |
| solid, wound outward | a hive's or grid's walls, or a scaffold's struts, clipped to the cavity | — |

Nothing is welded, so the model's triangles never pass through the cavity and the outside
keeps every imported detail. The positive winding rule does the arithmetic: `+1` in the
wall, `0` in the void, `+1` in a strut or a lattice wall, and `+2` where struts overlap at a
node (ADR 0071).

## The cavity

The isosurface `d = -t`, extracted from a field built around *that* surface rather than
the mesh (ADR 0058), so the cost is the mesh's own field whatever the wall thickness. What
comes out is clustered onto the lattice, so a smooth cavity is a few large facets rather
than a triangle per voxel face (ADR 0084); its triangle count follows its own shape.

A wall thicker than the model leaves the field empty, `extract` returns nothing and the
model comes back unchanged with `cavity_mm3` at zero — the honest answer to "hollow this
3 mm spar with a 2 mm wall", and the CLI reports it as a defect.

`External` is the same build at `iso_mm = +t`: a mould. Infill and blockers do not apply.

The fields are built from the shells that bound the solid, not from every shell of the
mesh. A shell most of whose faces have the rest of the mesh standing just outside them — a
stray part left inside a sculpt — is left out of them. A field over it reads its outside
as air and cuts a wall around it. It stays in the model, so under the non-zero rule it
prints solid wherever it is, in the wall or standing in the cavity. A mould turns the
bounding shells inside out, not the stray ones, which would print as resin in its void
(ADR 0186).

An open floor carries the cavity out through the floor the model stands on (ADR 0313).
`HollowSettings::open_toward` is the way to the plate in the space the wall is measured in,
`plate_down` of the placement, so it follows the model's turn and a shell turned after it was
built is stale. `carve` reads the cavity's outline a band in from the floor's inner face —
`floor − wall − band`, where it is the cavity's own — and sweeps it along that direction to a
band past the floor's outer face: `max(h − s, s − e, cavity(q))`, `q` the point carried back
onto the outline's plane, taken with `min` into the cavity before the blockers, so a blocker
still keeps the floor solid where it stands. The model's own floor stays in the mesh; the
cavity outweighs it there, so the rasteriser fills nothing, and what the swept void reaches
past the floor stands in air and prints as nothing. A floor that is not level opens only
where the outline reaches it: a ball opens in a disc at its bottom. Only a shell has a floor;
a mould ignores the direction.

## Blockers

A ball of radius `r` swept from `from` to `to` inside which the wall stays solid, kept in
model space so it travels with the model, subtracted from the cavity before anything is
meshed: `max(cavity, r - |p - segment|)` over the cavity's tiles and the capsule's own.
They keep a cavity out of a thin spar or a spike that would print hollow and snap, and out
of the sleeve around a channel. A click drops a ball at the inspector's radius — both ends
the same point — and an alt-click removes the one the click landed in.

## The lattice

| Pattern | What it is | Its numbers |
|---|---|---|
| Hive | hexagonal cells standing on the plate | hexagon diameter, wall, crossing holes |
| Grid | square cells standing on the plate | cell width, wall, crossing holes |
| Scaffold | struts on the edges of a cubic lattice stood on a corner | cell, density |

Each pattern is written down twice: as closed meshes on the global lattice — prisms for
the cells, cylinders for the holes, square bars for the struts — and as a closed-form signed
distance `solid(p)` (ADR 0302). The meshes are what a run cuts with, exactly, through
`core-csg` (`csg.md`): walls come out flat, holes round to `ROUND_TOLERANCE_MM`, and the
faces follow the cells rather than the lattice the cavity was cut on.

The shapes come in groups whose shells never cross, so each group is one solid the engine
reads. In every pattern the cavity is the void, whole, and what the lattice leaves standing
in it is appended as solid.

- **Walls.** A hive's or grid's walls are the cavity less its cells, then less each group of
  holes in turn, so they are one solid and a hole is an opening through it. A void built
  from cells and holes appended instead is right for the fill rule, but every cell's face
  is still drawn across the hole it meets, a membrane in the viewport that never prints. A
  hole reaches into the cells either side of its wall, half an inradius in a hive and a
  quarter of a cell in a grid, so no end of it lies on a wall's face.
- **Struts.** A scaffold's groups are each cut from the cavity and appended, not unioned:
  they overlap at the nodes, and the fill rule unions them as it does a support's struts
  (ADR 0041). The struts along the first axis run whole and the other two stop at each node,
  so the overlaps are small. Unioning them is what a lattice cannot afford: a scaffold
  crosses itself at every node, and on a 140 mm block that union took 17 s against 0.5 s
  for the three cuts.

The cavity's mesh comes from surface nets, which branches where two sheets of it touch along
an edge; `Solid::new` splits those seams (`csg.md`), so it is almost always a solid the engine
reads. Where it is not, the lattice is cut from the field instead: `max(cavity, -solid)` over the
cavity's tiles, as the blockers are, extracted once. That answer is the cavity's own
lattice, rough where walls meet, and it fills the cavity's volume rather than its surface,
so it is priced against the budget first and refused with `TooFine` rather than run over it.

**Hive.** Hexagons of diameter `D` across their corners, on a triangular lattice whose pitch
is the hexagon across its flats plus the wall, `sqrt(3) D / 2 + t`. A point is measured
from the nearest centre, which is one of the four corners of the lattice rhombus it stands
in, and the cell is the largest of its three distances along the side normals less the
inradius.

**Grid.** Walls `t` thick on `x, y = k (c + t)`, so the cells are `c` across.

**Crossing holes.** No cell of a hive or grid is closed: a round hole runs along the line
between every two neighbouring centres, through the middle of the wall they share. Rows
stand `first_hole_mm` over the cavity's floor and then every `hole_spacing_mm`, and none
below the first, so the walls stand whole on the floor and every hole is an arch rather
than a slot. A hole as wide as the cell would cut the junctions away and is refused. All the
cells of a cavity are then one pocket, which one drain hole empties (ADR 0189). A sliver of
a cell at the rim lower than the first row has no hole of its own; the drainage check
reports it.

**Scaffold.** The struts are square, `x c` across, on the edges of cubes of side `c` turned
so the body diagonal stands upright: every strut climbs at `asin(1/sqrt(3))`, 35° off level,
so none starts in mid-air and every node is held from below. Three square bars through a
cube fill `3x² - 2x³` of it exactly, so the density a user states is inverted by halving
for `x`.

**What it costs.** The walls cost what their holes cost: each group is cut through the
whole of the walls cut so far, one after another, and the engine's cost follows the faces of
both sides. Two things keep that down. A shape whose box meets no tile of the cavity's field,
band or inside, is never meshed; on a curved model much of the lattice's box is air. And the
cavity is halved across its longest side four times over — sixteen pieces, two for each of
eight threads, fixed so that every machine carves the same mesh — no piece under four reaches
across, each half the engine's own intersection and difference with one box.
The pieces are carved in parallel and appended; two halves meet face to face on the cut,
which the fill rule reads as one solid. A plane cut through `core-geometry` would be cheaper,
but it leaves loops open at the cavity's seams. A halving the engine refuses carves that
piece whole.

Measured on a 140×70×80 mm block with a 2 mm wall at precision 0.5, 6 mm cells and 2 mm
walls, before the culling and the pieces, the lattice's share of the run:

| | Field | Exact | Faces, field against exact |
|---|---|---|---|
| Hive | 3.2 s | 5.0 s | 2.45 M against 0.70 M |
| Grid | 2.4 s | 1.5 s | 1.73 M against 0.40 M |
| Scaffold | 3.2 s | 1.4 s | 4.21 M against 0.31 M |

Appending the holes to a void of cells instead took 0.9 s for the hive and 0.5 s for the
grid there, and drew the membranes above. And by `cargo bench -p core-volume --bench field -- infill/` on a 100 mm ball, the same
settings, the exact cut whole against culled and in pieces:

| | Whole | Culled, in pieces |
|---|---|---|
| Hive | 5.9 s | 2.4 s |
| Grid | 2.3 s | 1.3 s |
| Scaffold | 1.0 s | 1.2 s |

A scaffold's three groups were already cut in parallel, so on the ball the halving is what it
pays for. On a 406k-face bust, whole against in pieces: hive 5.5 s and 2.4 s, grid 2.1 s and
1.5 s, scaffold 1.6 s and 1.2 s.

## Precision

One number, 0 to 1, for how smooth the cavity comes out, as every resin slicer states it.
It moves the cavity's lattice geometrically from 0.8 mm to 0.05 mm, because cost is its
square, and it is capped three ways: a wall always gets three voxels, nothing is finer than
`sqrt(area / 7e6)`, and what survives is priced against the budget before a voxel is
filled. It is not the layer height — the outside never passes through the lattice.

The area cap is what makes precision mean one thing on every model. A spacing in
millimetres alone costs four times as much on a model twice as long; a cap on the longest
side instead gives a spire and a block of that height the same spacing and nothing alike in
cost (ADR 0083). `MIN_WALL_MM` is what the finest lattice can carry three voxels across,
and it is the only floor a wall has.

## The budget

`HollowSettings::budget_bytes`, 1.5 GB by default, is what a run may take. `build` prices
the tiles the band will reach before filling any, and over budget the run is not attempted:
the lattice is coarsened to the spacing that would have fit, a twentieth coarser again, and
retried, up to three times. So a run ends in a cavity, never in an exhausted machine, but
it cannot promise the lattice asked for — `Hollowed` carries the spacing it settled on and
whether it coarsened, the CLI prints both and the window says so (ADR 0063).

The budget covers the field and the cavity's mesh. What is downstream — the contours a
dense lattice puts on every layer, the stack, the file — is not priced, so a honeycomb of
1 mm cells still costs what that honeycomb is.

## Drain holes and channels

A hole is a cone appended to the whole solid wound inward, so it cuts the shell and the
cavity's ceiling in one pass, and it is exact at any layer height: `core-volume/drain.rs`
meshes it, nothing about it is voxelised. Its mouth starts a radius clear of the surface —
more than the sag of any surface curving away under a hole that wide — so the mouth is
never left capped, and what stands outside the model prints as nothing. Every body is laid
`CUT_WEIGHT` deep, so it takes out the lattice bonded into the wall and a support's tip with
the shell rather than leaving them standing in the hole (ADR 0188).

| | What it is | Where it comes from |
|---|---|---|
| `DrainHole` | mouth on the surface, axis into the model, diameter, depth, taper, outline (round or a polygon of 3 to 12 sides, turned from the level), through or blind | a click in the window, `--drain-at` in the CLI |
| `Channel` | a tube along a polyline, a ball at each bend | clicks in Channel mode, `--channel` |

Holes and channels live in the model's own space beside the blockers, and are cut on their
own rather than by the hollow run: `drill` meshes the bodies and whoever is about to slice
or draw the model appends them (ADR 0075). A cut needs no cavity, so a click opens a hole
the same frame, and the shell does not go stale behind it. A hole placed on the plate is
measured there: both its diameter and its depth come back through the model's scale.
`hole_at` puts one on the surface nearest a point, which is how the CLI places a hole
without a cursor.

A hole's own volume is not taken off the resin saving: it is a cone through a wall, and the
number it would move is smaller than the cavity's own rounding.

Two things make a hole actually open. Its mouth starts `MOUTH_LIFT_MM` past the surface
plus whatever that surface rises around the rim — `lift_for` casts a ray out of the
material along the hole's axis at twelve points around it — so no film is left over the
mouth. A ray that meets air first counts nothing, so another part of the model standing over
the hole is never cut.
And `pierce` deepens a hole that goes through, standing on a hollowed model, until it is
through the wall, `thickness + 2 voxels`, whatever depth was asked for: a hole shorter than
its wall is a dimple, not a drain, so the depth a user types is a floor rather than a
ceiling. A blind hole is the other answer, a socket for a magnet or a pin, and keeps the
depth that was typed; so does any hole on a solid model, which has no wall to cross.

A polygon's corners lie on the hole's diameter and its turn is measured from the level line
`frame` lays the first corner on wherever the axis is not upright, so a user turns a corner
to the top of a hole in a side wall and the roof prints as a point rather than a flat span.

A plug is what a hole took out of the wall, cut as its own closed mesh rather than through
the boolean engine (ADR 0313): the hole's outline moved in by the gap — by `gap / cos(π/n)`
at a corner, so every side clears by the gap — and as long as the wall where the model is
hollow and the hole goes through, as deep as the hole otherwise. Its inner end is flat. Its
outer face is sampled on four rings over the mouth, each point dropped onto the model's
surface by a ray cast out of the wall the way `lift_for` reads a rim, so it closes flush on
a curved surface; a point no ray finds reads the mouth's own plane. `stand_on_end` turns it
onto that flat end beside the model. The gap a user sets is the fit that prints, so a resin
whose tolerance compensation is off has its hole and outer offsets taken off it
(`Compensation::fit_drift_mm`).

A channel runs exactly between the points it was given, which is what `--channel` hands
over. A run of clicks is not those points: `channel_under` sinks the spine a radius and a
mouth's clearance under every point it was clicked at, and only the two ends climb back out
to open on the surface. A spine left on the surface would be tangent to it and cut an open
groove along the model rather than a tunnel under it.

A channel is a pipe through the part, so the cavity has to keep off it: `sleeves` turns
each leg of the spine into a `Blocker` of the tube's radius and the wall, and the window
and the CLI pass those in with the blockers a user placed (ADR 0076). The wall of the pipe
is therefore the wall of the model. A channel dug or cleared on a model that is already
hollow rebuilds its shell at once, at the numbers it was built at, and a sleeved channel
drains its own two mouths, not the cavity it crosses.

That rebuild does not build the field again. A run is `cavity_field` then `carve`, and the
field holds neither blockers nor infill, so the window keeps the fields of the models it
hollowed lately and carves a kept one again: 0.4 s rather than 5.4 s on a 406k-face bust.
They may hold 512 MB together on the desktop and 128 MB in a browser, a field over that is
never kept, and in a browser they make way for a field being built or a plate being sliced
(ADR 0260).

What the window draws inside a cut is the cut's own body. Nothing meshes the surface of a
hole: the bodies are already there, wound inward, so the face that survives culling is the
far wall of the tube — the one a hole is looked into. Four passes resolve it per object:
the bodies into the depth plane, the object itself counted into the stencil against that
depth, the bodies again wherever the count says material stood in front of them, and once
more to take their depth back out where it did not. The opening is therefore closed by a
silhouette resolved per pixel, and nothing approximates where the tube meets the surface.
See ADR 0201, and `local-docs/PLAN-viewport-cuts.md` for what the alternatives cost.

## Trapped resin

A cavity with no way out prints full of uncured resin. `core-supports` finds those pockets
on the stack rather than in a field: `TrapScan` takes layers in print order, reads the air
between the material runs of each one, and joins the runs that touch across a row or across
a layer. A pocket that reaches the edge of the grid, the first layer or the top of the
stack has somewhere to drain; one that closes without ever doing so is trapped, and its
lowest layer is where a hole goes. Each one carries what it holds, where its hole goes and
the box it fills. See ADR 0072 for why it lives there and what the grid
costs it.

The CLI folds the scan into the pass that writes the file, and reports each pocket with
what it holds; `--strict` fails on one. The window checks a model at a time on a worker
thread, when the Drain panel is asked to and at no other time, and offers to drill a hole
into every pocket it found, from the nearest surface and deep enough to reach it. A hollow
run, a hole or a channel leaves the last check out of date rather than starting another,
and a run drops what the last one found, since the cavity it stood in is a new mesh
(ADR 0198). A model a pocket was found in has its cavity painted red in the viewport
wherever a pocket stands in it — `Hollowed::cavity` is the face range of the shell that
bounds the lot, and each pocket's own box is what the shader keeps the red inside
(ADR 0190, 0200). The x-ray of `viewport.md` is what shows it, turned on by hand.

## What it does not do

- A hollowed mesh is not watertight and `diagnose` says so. Nothing downstream of the
  rasteriser asks, but it cannot go back into `build`: the winding number would read the
  cavity as a second shell. Step 8e cuts the cavity's *field* instead.
- The shell is kept in model space but measured on the plate, at the scale the model
  stood at (ADR 0185). A model scaled after hollowing has a wall scaled with it; the window
  notices and asks to be run again.
- A channel narrower than a tenth of a millimetre drains in life and reads as closed to the
  scan, so the pocket behind it is reported. The error is a hole nobody needed.
- The window draws an open floor closed: the model's floor is still in the mesh, and only
  the layers show it gone. A floor on the plate seals the cavity until the print comes off,
  which the drainage check reports as a suction cup it is.
