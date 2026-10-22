# 0330. Touch every island where it begins, and sit every tip on its face

- **Status:** Accepted
- **Date:** 2026-10-22

## Context

Placement reads the stack on 0.1 mm cells (ADR 0032), and a lobe hanging off a finger is a
few tenths across for its first layers. Such a piece is seen a layer or two after it
began, as an island sampled at its rim or as a speck that merges into a held part before it
is more than dust, so its contact goes beside where it begins and higher. A tip that bites
0.2 mm hid some of this; with the shipped bite of nought (ADR 0323) the contact touches the
surface above the lobe's first layer, which cures on the film. One figure stood eleven
islands of 0.01 to 0.14 mm², each with a contact 0.3 to 0.9 mm away; the check the window
runs found every one.

The supports that steady a part held at too few places (ADR 0327) go on the outline of the
part's whole section, a layer under it: inside the part wherever its wall stands straight
or leans out. A contact inside never reaches a face over it, so ADR 0323's move up leaves
it there, and the head and pillar are built through the part.

A plane tip ends in a disc square to its head (ADR 0243), and a head leaves its face along
the normal only where nothing is in the way (ADR 0249, 0328). On that figure half the
heads lean more than 15 degrees off their face, a fifth more than 30: with no bite, such a
disc meets the face along an edge. A trunk standing on the part at 45 degrees ends the same
way, and the check read one such landing as a 0.02 mm² neck under 27 MPa.

An island a few tenths across that begins whole, a lip with no low point, was held by
one support at the end of it: the check is content with any pixel under it, and the rest
hangs off that pixel.

## Decision

**Every island is read again finer.** Once the climb is done, `hold_islands` reads every
layer on 0.02 mm cells, finer than any printer's pixel, and takes the pieces touching
nothing of the layer under them. One under 0.005 mm², half the least the check names, is
dust and taken out of the layer under the next, so what grows from it is an island in
turn. An island is touched when a contact stands on its seat — what of it lies 0.06 mm,
half the thinnest tip's radius, in from its rim, or all of it where nothing does — within
two layers under its plane. One no contact touches is held at the lowest low point on its
seat, or the seat's cell nearest its middle, a layer under the plane. A contact the run
placed on the island's rim is moved there, it being this island's own; failing one, the
nearest within 0.3 mm across, at the island's height or up to as far above; failing that
a contact is added. Islands are taken bottom up, so a contact once touching one is never
moved off it. A painted-out patch and the plate's own height are left alone.

**A piece held at one place is held at its middle.** Placement samples a piece outline
first, corners first; a piece too small for two supports kept the one corner, so a lip
was held at its end and hung off it as a lever. It now gets the cell nearest the middle of
its room instead, or of itself where it has none.

**A contact inside the part is put on its underside.** Building a column, a contact whose
ray straight down leaves the model through a face looking down is moved to that face,
whatever the bite. ADR 0323's move up is asked only of a contact outside.

**A plane tip's disc is laid on its face.** The mesher moves each point of the tip's first
ring along the head's axis onto the plane of the face the contact sits on, the one
`tip_direction` leaves; the disc becomes an ellipse as wide as the contact across the lean
and lies on the face whole. The tip a trunk standing on the model ends in is laid the
same way on the face its landing ray met. An axis leaning past 75 degrees off the normal
keeps its disc square, and no point moves further than halfway to the ring beside it.

## Consequences

The figure that stood eleven islands stands none in five poses, for about one more contact
an island; the steadying supports on its fingers meet their underside instead of curing
through it, and a leaning tip meets its face over its whole end. The ellipse is larger than
the contact by one over the cosine of the lean, 1.4 times at 45 degrees, and so is its
scar. A lobe beside another may take two tips a quarter of a millimetre apart.
Placement costs about 2.5 times what it did on the `place` benchmark (tables 8 to 23 ms,
ball 12 to 30 ms): the fine read is the coarse one's order again, layer by layer in
parallel, with a layer whose new material all stands beside the old skipped. Reopen if a
large plate's run is felt to stall, by reading only windows round the layers' small
outlines. A contact deep in a solid block, which used to be dropped, now holds the
underside under it.

## Alternatives considered

### Hold every low point

No second read, but most low points are bumps on an underside something already holds:
95 of one figure's 220 hang, and eleven begin an island.

### Read windows round the low points only

Cheaper, but an island that begins on a level edge or a small flat face has no low point,
and a model drawn upright has many.

### Move a nearby contact to the island's lowest point, or to its nearest cell

Tried first. Along a level lip the lowest vertex is an accident of the mesh, and a contact
moved half a millimetre there left the end of the lip it held; moved only to the nearest
cell, it stayed on the rim, which holds a lip by its end.

### Lift the bite for biteless tips, or keep every head on its normal

A bite hides the lean, but nought means nought. A head kept on its normal runs into the
model wherever ADR 0328 turns it.

### The option that won, and what it costs

A second pass over the stack at five times the coarse grid's rows, supports placed by a
rule the climb does not know about, which a second run then counts as seeds, and tips
whose scar grows with their lean.
