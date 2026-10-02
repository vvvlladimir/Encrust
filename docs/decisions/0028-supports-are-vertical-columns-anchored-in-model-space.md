# 0028. Anchor supports in model space, build them as vertical columns

- **Status:** Superseded by 0125 (the tip's direction only)
- **Date:** 2026-09-18

## Context

A support is placed by clicking the model, which gives a point on its surface in plate
coordinates and the normal of the face that was hit. From that a column has to be built,
and it has to survive the user moving, turning and scaling the model afterwards.

Two questions follow, and they are not independent.

The first is which space the contact is remembered in. Plate coordinates are what the
column is built in and what the slicer wants. Model coordinates are what keeps the contact
on the same piece of surface when the model is turned.

The second is which direction a column runs. The surface normal is the direction the
support "should" leave the model along, and is what the tip would penetrate against. The
plate's own up is the direction gravity and the peel force actually work along, and the
direction the printer builds in.

The mesher also has to produce a closed solid every time. A slicer that is handed an open
or self-intersecting mesh produces a broken stack, and the mesh it is handed here is one
this project generated, so there is no excuse for it.

## Decision

A `SupportPoint` holds its contact in the **model's own space**. Everything derived from it
is rebuilt from the model's placement whenever that placement changes.

A `Column` is **vertical in plate coordinates**. Its axis runs from the contact straight
down to whatever is under it, found by casting a ray along `-Z` from a micrometre below the
contact and taking the plate at `z = 0` when nothing is hit. The tip cone points straight
up into the model by `contact_depth_mm`, not along the surface normal.

The column is meshed as a solid of revolution about that vertical axis: an apex, then
horizontal rings at the contact, at the foot of the neck, at the bottom of the pillar and,
when it reaches the plate, around the foot, then a flat disc underneath. Rings are pushed
through a filter that keeps them non-increasing in Z and drops any that repeats the one
before it.

Contacts with less than `MIN_PILLAR_HEIGHT_MM` of room under them produce no column at all.

## Consequences

Moving, turning or scaling a model carries its supports with it and re-lands every one of
them, so a model dropped to the plate keeps supports that still reach the plate. A
non-uniform scale moves the contacts but not the column thickness, which is right: the
profile describes what the printer can print, not what the part is.

A vertical column cannot invert itself. The apex is always `contact_depth_mm` above the
contact ring and the rings always descend, so the lathe is closed and outward-wound for any
contact and any profile. The `diagnose` tests in `core-supports` pin that down for a column
on the plate, a column landing on the model, and a column too short to have a neck.

The surface normal is thrown away. A support on a nearly vertical wall bites upward rather
than into the wall, which is a weaker anchor than a normal-aligned tip would give. This is
what a default SLA tree does too, and it is the right trade for MSLA,
where the column has to be vertical anyway for the peel force to pull along it.

Rebuilding costs one raycast per point over the whole model mesh, every time the placement
changes. With manual placement that is tens of rays. Reopen this when automatic placement
in step 7b makes it thousands, with a benchmark that says how much it costs.

## Alternatives considered

### Remember the contact in plate coordinates

Simplest: the contact is already in plate coordinates when the click produces it, and no
column ever has to be rebuilt for a move. Rejected because the supports then stay where
they were while the model slides out from under them, which is wrong the first time anyone
nudges a part.

### Run the column along the surface normal

A tip driven in along the normal grips a sloped surface far better, and it is what an
FDM-oriented generator would do. Rejected for two reasons. The column still has to reach
the plate vertically, so a normal-aligned tip needs a bend, which is a second piece of
geometry and a second set of degenerate cases. And on a near-vertical face the normal is
nearly horizontal, which turns the tip cone into a flat spike that self-intersects the
pillar.

### Vertical columns anchored in model space, which is what we do

The honest costs are the two above: a weaker grip on steep faces, because the tip bites
upward rather than into the surface, and a full rebuild of every column whenever the model
moves. The rebuild also means the supports flicker while a gizmo drag is in progress on a
large mesh, because the rays are cast again on every frame of the drag.
