# 0122. A relief's depth is a plate millimetre

- **Status:** Accepted
- **Date:** 2026-09-28

## Context

`press` takes an amplitude in the millimetres of the space the mesh is in. That space is
the file's, and a file's unit is whatever its author worked in: a statue exported from 3ds
Max is 1.33 units tall and prints at 140 mm, a tank is 9.9. Pressing 0.3 mm into the statue
as it arrives moves a quarter of the model, and the lattice — which is chosen from the
amplitude — comes out coarse enough to leave five hundred triangles where there were forty
thousand.

The same gap is in the window, where a model is scaled where it stands and the depth is
typed into a panel that says `mm`.

## Decision

A depth is a plate millimetre, and so is the lattice the relief is cut on. The CLI presses
after `place`, so the mesh is already scaled, centred and turned; the window scales the mesh
by the model's own scale before pressing it and clears that scale afterwards, since the
relief is now in the geometry.

Scaling the depth instead would have been smaller, and wrong: the lattice has a floor of
0.05 mm, so a statue drawn 1.33 units tall would be cut into twenty-six voxels whatever the
depth said, and come back as a lump.

The press still runs before `orient_outward`, which rewinds faces the map is indexed by.
Placement does not: it moves every vertex and touches no face.

`press` returns the lattice it used and whether the memory budget made that coarser than
precision asked for, and both the CLI line and the window's status say so. A relief that
came out blunt because the field would not fit is otherwise indistinguishable from a relief
that came out blunt because the texture is flat.

## Consequences

A pressed model has no scale left on it: what was a scale is vertices now. Scaling it again
afterwards scales the relief with it, which is what a relief in the geometry means.

A model scaled unevenly is pressed as it stands, so a depth is along the surface's own
normal of the stretched model rather than of the drawing.

A relief is now pressed into the oriented, placed mesh, so changing the placement after
pressing does not change the relief: it is geometry, and it scales with everything else.

## Alternatives considered

### Divide the depth by the model's scale

One line, and it needs no copy of the mesh. It lost to the lattice: `lattice_mm` clamps to
0.05 mm, which is a fifth of a model drawn in metres, so the field would be coarser than
the part however small the depth was made.

### Keep the amplitude in the file's own units

What `core-volume` naturally speaks, and what hollowing does with a wall thickness. It lost
because a depth is the one number a user reads off a printed part, and no one knows what a
unit of a downloaded OBJ is worth until they scale it.

### Scale the mesh into millimetres at load

Would fix this and the wall thickness at once. It lost because a file carries no unit to
scale by — this is exactly why the numbers differ — so it would mean guessing one.

### The option that won, and what it costs

A scaled model is copied once more before it is pressed — a mesh of tens of megabytes twice
over for the length of the run — and its hierarchy is rebuilt over the copy.
