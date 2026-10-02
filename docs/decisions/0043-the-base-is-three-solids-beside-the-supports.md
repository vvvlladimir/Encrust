# 0043. Build the raft, the skate and the braces as solids beside the supports

- **Status:** Accepted, with the skate's cock and the raft's slope superseded by 0044
- **Date:** 2026-09-19

## Context

Step 7e adds the three things that hold a supported plate down and stop it swaying: a
raft under the feet, a skate foot that a blade can get under, and cross braces tying tall
trunks to each other.

None of them fits the vocabulary the mesher has. `docs/decisions/0041` sweeps rings about
an axis, and `docs/decisions/0042` said in as many words that the skate had no axis of
revolution and would have to wait for a second primitive. A raft is a slab over a
footprint, which is not a support at all. A brace runs between two supports and belongs to
neither.

The raft raises a second question. In most slicers a raft lifts everything above it: the
supports start on the raft's upper surface, so every landing moves. That is a change to
`Landing`, to the height check in `columns`, and to everything the preview and the `.goo`
writer see.

The workspace still has no polygon boolean engine, by `docs/decisions/0032`, so a raft
footprint cannot be an offset union of the feet.

## Decision

All three are **solids appended beside the supports**, in a new `core-supports::base`
module, and `mesh_trees` appends them after the trees. They are never welded to anything,
because the non-zero winding fill unions overlapping solids; see `docs/decisions/0041`.

**The raft does not lift anything.** It is a slab from `z = 0` to `raft.thickness_mm`, and
the supports keep landing on the plate and pass straight through it. A foot inside the
raft does nothing, which is right: a plate with a raft does not need feet. `Landing`, the
height check and everything downstream are untouched.

Its footprint is the convex hull of the feet that reach the plate, or their bounding
rectangle, by `raft.shape`. The hull is Andrew's monotone chain over the foot centres —
about forty lines, which is what a polygon dependency would have cost many times over for
this one use. The outline is then pushed out by one foot radius, so a foot is covered
rather than merely touched, and spread about its own middle by the square root of
`raft.area_ratio`, so the ratio names an area and not a length. `raft.slope_deg` leans the
walls out on the way down, so the slab meets the plate wider than it ends.

**A skate is a closed box cocked up at the toe**: eight corners, twelve triangles, a
bottom quad whose toe end rides `SKATE_COCK` of the thickness off the plate. Its toe
points **away from the part the support came down from** — the horizontal direction from
the tree's highest tip to its root — so the blade goes in from outside. A support that
never leaned has no such direction and takes the plate's own x.

`PlatformShape::is_round` is what tells the mesher whether the foot is swept as rings or
built as a skate. Every other platform shape stays a ring sweep.

**A brace is a tube between two trunk axes**, at the same height on both. Trunks within
`bracing.max_spacing_mm` of each other are tied every `bracing.rise_mm` from
`bracing.start_height_mm` above what they stand on, as high as both of them still stand —
which is the lower of the two root nodes, because above that a trunk has already split
into branches that a horizontal strut would miss.

All of it is off in the shipped presets. A plate that does not ask for a raft, a skate or
a brace meshes to exactly what step 7d produced.

## Consequences

The mesher gained one primitive and one module, and nothing else moved. `Landing`, the
branching solve, the small pillar rule and the preview are all untouched, and the tests
for them still pass unchanged.

`crates/core-supports/src/base.rs` pins down what each piece has to be: a skate, a raft of
either shape and the whole plate with all three switched on are closed, manifold and wound
outwards; the raft reaches every foot; a bigger area ratio is a bigger raft; the walls
lean; braces are horizontal, start no lower than asked and step by the rise; and a trunk
too short, too far or switched off is not tied.

A raft that does not lift means the supports' own feet are buried in it and their material
is wasted. The answer is to pick `platform_thickness_mm` at or under `raft.thickness_mm`,
or to accept the few cubic millimetres. If someone needs the feet gone, the fix is a rule
that skips the foot when a raft is on, which is a line in `stand_on` — not a change to
this decision.

The hull is over the foot **centres**, widened by one radius. That is exact for a circular
foot and a little tight for the far corner of a cube or the toe of a skate, by up to the
difference between a foot's circumradius and its inradius. At the shipped 8 mm foot that
is under two millimetres, inside what `area_ratio` adds back. Reopen it with a picture of a
foot hanging off the raft.

Bracing is O(n²) over trunks with no spatial index, unlike the merge in
`docs/decisions/0040`. On the plates this has been run on that is a few thousand pairs and
does not show up. Reopen with a benchmark if a dense plate makes it show.

## Alternatives considered

### Lift everything onto the raft, as most slicers do

The honest model: the raft is the base, and the supports stand on it. Rejected for this
step because it moves every landing, and with it the height check that decides whether a
contact has room at all, the preview's stack and the numbers the inspector reports. That
is a change to the pipeline for a feature whose whole job is to add material under the
feet — and `docs/decisions/0041`'s union rule already makes the cheap version correct.

### Offset and union the feet into a raft footprint

What a slicer with a polygon library does, and it gives a raft that hugs the supports
instead of a convex slab. Rejected because the workspace has no such library by
`docs/decisions/0032`, and a raft is the one place where a hull is arguably *better*: a
concave raft peels unevenly, which is what the raft was there to stop.

### Three solids beside the supports, which is what we do

The honest costs are the three above: the feet are buried and wasted, the hull is measured
from foot centres and can be a millimetre tight at a corner, and bracing has no spatial
index. There is a fourth. Because none of the three is welded, a raft and a skate overlap
along a face rather than sharing one, so the support mesh self-intersects more than it did
— harmless under the fill rule, and one more reason `signed_volume` of a support mesh
over-reads.
