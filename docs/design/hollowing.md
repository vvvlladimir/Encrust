# Hollowing and infill

How a solid becomes a shell and what fills it. `core-volume`: `hollow.rs` holds the modes,
blockers and assembly, `infill.rs` the lattices. Both stand on `volume.md`.

Why: ADR 0058 (band on the isosurface), 0059 (cavity appended, not welded), 0060 (a
lattice is boxes), 0063 (the budget), 0083 (the precision ceiling), 0189 (a hole is what
drains a cavity, and no cell is closed).

## What comes out

`hollow` returns the whole solid ready to slice, plus the volume the cavity saves: three
meshes appended in one buffer and nothing else.

| | Internal | External |
|---|---|---|
| outside | the model, byte for byte | the surface grown by `t` |
| cavity, wound inward | the surface `d = -t` | the model, wound inward |
| infill, wound outward | boxes of the lattice inside the cavity | — |

Nothing is welded, so the model's triangles never pass through the lattice and the outside
keeps every imported detail. The positive winding rule does the arithmetic: `+1` in the
wall, `0` in the cavity, `+1` in a strut standing in it, `-1` and so still air where
something wound inward reaches past the model (ADR 0071).

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

## Blockers

A ball of radius `r` swept from `from` to `to` inside which the wall stays solid, kept in
model space so it travels with the model, subtracted from the cavity before anything is
meshed: `max(cavity, r - |p - segment|)` over the cavity's tiles and the capsule's own.
They keep a cavity out of a thin spar or a spike that would print hollow and snap, and out
of the sleeve around a channel. A click drops a ball at the inspector's radius — both ends
the same point — and an alt-click removes the one the click landed in.

## The lattice

| Pattern | What it is | What it carries |
|---|---|---|
| Hive | hexagonal tubes standing on the plate | load up and down |
| Grid | square tubes standing on the plate | load up and down |
| Scaffold | struts along all three axes of a cubic lattice | load sideways as well |

All three are open networks, because a closed cell traps resin and a hollow that traps
resin is worse than none; the vertical two drain through their own tubes and through the
openings below. No gyroid — it is the only pattern that cannot be written as boxes
(ADR 0060).

Nothing is voxelised. Every piece is a closed box of twelve triangles whatever the
precision, and boxes overlap rather than weld, since the fill rule unions them, as a
support's struts do (ADR 0041). Cost follows the cell count, which is thousands, not the
resolution, which has no bound.

### Density, not thickness

The user states how much of the cavity is filled; the wall follows by inverting each
pattern's solid fraction.

| Pattern | Solid fraction | Wall for a density `f` |
|---|---|---|
| Grid | `1 - (1 - t/c)²` | `t = c (1 - sqrt(1-f))` |
| Hive | the same, on the hexagon's inradius | `t = (sqrt(3)/2) c (1 - sqrt(1-f))` |
| Scaffold | about `3 t² / c²` | `t = c sqrt(f/3)` |

`c` is the cell — a side for grid and scaffold, the circumscribed diameter for the hive,
which is how slicers state it. A 20% grid of 5 mm cells is a 0.53 mm wall. Grid and hive
are exact; the scaffold ignores the overlap at its nodes, so at 30% it comes out a few
percent denser than stated. The openings below take `2 OPENING_MM / h` off a wall of height
`h` — 6 % in a 26 mm cavity — so hive and grid come out that much lighter.

### Clipping by sampling

The cavity still bounds the lattice, but it is sampled rather than meshed. A vertical wall
runs along one segment of the footprint, walked in chunks; each chunk asks the field for
the stretches of its own vertical line inside the cavity, sampled at the lattice step and
cut where the sign changes. A chunk's spans are taken at both ends and the middle and
**intersected**, so a box is only ever shorter than the cavity above it — a box reaching
out through the wall would be a contour with nothing around it, which the fill rule turns
into material in mid-air. Each end is then pushed back out by half the wall thickness so
the box fuses into the shell, still inside the model because half a wall is half a wall.

A scaffold strut is a straight line, so its spans are its boxes and it needs no chunking.

A wall also has to reach the side of the cavity it runs into, and a chunk that leaves the
cavity holds no span, so the walk stops up to a chunk short of it. `meet` takes that last
stretch a lattice step at a time, which follows a side curving away under or over the wall,
and then past the last step that held to where the side is at the middle of the wall's own
height, plus the bond. Without it a corridor of air runs along the cavity's wall and joins
every outer cell into one pocket (ADR 0189).

### The openings

A hive or grid cell would otherwise be a closed box. Each wall leaves `OPENING_MM` = 1.5 mm
clear over the floor of the cavity and under its ceiling, except within `POST_REACH` — a
quarter of a cell — of a junction, where it stands the whole height and welds into the
shell. So a post holds every junction, the open middle half of each cell side is a bridge
short enough to print, and the resin runs from cell to cell to whatever hole was drilled.
The opening is taken off the spans before the bond is added back, which measures it from the
surface the wall welds into; a cavity shallower than twice the opening plus the bond keeps
the posts alone, with no wall between them.

The open wall is **one walk along the whole segment**, so it merges exactly as it did before
there were openings; only the posts are stood stretch by stretch, as a pair of caps at the
floor and the ceiling of each span. That is what the cost follows. On the 60 mm ball above,
2 mm grid cells at 15 % and 0.05 mm layers: the walls alone come to 56 contours on the
busiest layer, and the posts bring that to 830, slicing in 97 ms against 13 ms. A hive pays
almost nothing for its openings — 1402 against 1226 at 5 mm cells — because its edges are
one cell long and never merged across cells anyway (ADR 0189).

**Merging the runs** is what makes it affordable. A wall's cross-section is the same on
every layer it crosses, so a box per chunk would put seven contours on a layer where one
does. A chunk's spans are merged into the run before them and the run is broken only where
holding it would cost more than a lattice step of height, so a wall crossing the middle of
a cavity is one box and only its ends, where the ceiling falls away fast, are chased chunk
by chunk. On a 60 mm ball with 2 mm grid cells: 83 contours on the busiest layer instead of
8 928, before the openings below add their posts. `precision` sets the chunk, a quarter of a cell at 0 down to a sixteenth at 1, so it
buys detail where the lattice meets the shell and costs nothing elsewhere.

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
| `DrainHole` | mouth on the surface, axis into the model, diameter, depth, taper | a click in the window, `--drain-at` in the CLI |
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
plus whatever that surface rises around the rim — `lift_for` samples the rise by casting a
ray down the hole's axis at twelve points around it — so no film is left over the mouth.
And `pierce` deepens a hole standing on a hollowed model until it is through the wall,
`thickness + 2 voxels`, whatever depth was asked for: a hole shorter than its wall is a
dimple, not a drain, so the depth a user types is a floor rather than a ceiling. On a solid
model there is no wall to cross and the depth is what was typed.

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

What the window draws inside a cut is a second mesh, `bores`: the wall of every tube and
the floor of every hole that ends in material. Both are clipped to the model with one ray
per sector — twenty-four along the tube for the wall and twenty-four out from the centre
for the floor, exact where they land and polygonal between them, which is what the tube
already is. A strip hangs between two of those lines and is drawn only where both of them
run through material, so a hole drilled across an edge of the model stops a sector short of
that edge instead of hanging a sector of itself into the air, and its floor is the half
disc the material leaves rather than a whole one. A hole's wall is cut short at the wall a
cavity left, so nothing of it crosses the cavity, and a hole that reaches that far is
through: it ends in the cavity and takes no floor, which is what keeps a drain from reading
as a dimple with a coin at the bottom of it. A channel runs in its sleeve, so its wall
goes its whole length, with a ball's wall round each bend. It is drawn and never sliced:
the cut itself is what the layer sees.

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
- Saved resin is reported as the density times the cavity, true by construction. The mesh's
  own volume cannot be used, because the boxes overlap.
- A channel narrower than a tenth of a millimetre drains in life and reads as closed to the
  scan, so the pocket behind it is reported. The error is a hole nobody needed.
