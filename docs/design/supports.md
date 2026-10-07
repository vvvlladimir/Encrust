# Supports

How a support gets from a click into the printed file. Why it is built this way:
ADR 0026–0029, 0031–0034, 0036, 0040–0044, 0092–0095.

## The pipeline

```
click ─> PlacedHit ─> SupportPoint ─> Column ─> SupportTree ─> Mesh ─> merge_visible
         (plate)      (model space)   (plate)   (plate)        (plate)
```

`pick_surface` casts the cursor ray at every visible object; `ObjectSupports::add` maps
the hit back through the model's transform and keeps it in model space; `columns` maps it
forward and lands it; `grow` merges tips into trunks; `mesh_trees` sweeps every strut into
one mesh; `merge_visible` appends it, so the slicer sees one solid.

The last three run again whenever the points, the placement or the profile change;
`ObjectSupports` remembers what it built against, which keeps the GPU mesh cache from
churning. Landing is one ray per point through the scene's `Bvh`, on every core (ADR 0036).

## Where a column lands

A tip leaves the surface square on: `columns` reads the normal of the face nearest the
contact and puts the **neck** — the far end of the `[top]` segment — one `top.length_mm`
out along it, straight down where that normal does not look downwards, and shorter where a
full one would eat the room the body needs over the plate (ADR 0125). Everything below is
measured from the neck, not from the contact.

A ray runs down from `SURFACE_EPSILON_MM` (1 µm) below the neck — starting at the surface
itself would report the contact's own face at zero distance.

- Hit above `z = 0`: the column lands on the model and its lower end sinks
  `contact_depth_mm` into that surface, so the two cure as one solid. A profile with
  `land_on_model` off refuses that hit, and the tip bends a knee or is dropped (ADR 0124).
- Otherwise it lands on the plate and gets a foot.

Less than `MIN_PILLAR_HEIGHT_MM` (0.2 mm) of room means no column. The point is kept — the
model may move up — and the inspector reports standing against placed.

## The shape of a support

The profile is grouped as MSLA slicers group it, so numbers tuned elsewhere carry across:
`[tip]`, `[top]`, `[middle]`, `[bottom]`, `[small_pillar]`, `[branching]`, `[raft]`,
`[bracing]`. An unmerged support is a solid of revolution about its own axis:

| Piece | From | To | Radius |
|---|---|---|---|
| tip cone | apex, `tip.contact_depth_mm` above the contact | the contact | `tip.contact_diameter_mm / 2` |
| top segment | the contact | `top.length_mm` along the face normal | `top.upper_diameter_mm / 2` to `top.lower_diameter_mm / 2` |
| pillar | the top segment | the top of the flare | the node's radius, `middle.diameter_mm / 2` unless it is a trunk |
| flare | the pillar | the top of the foot | `bottom.upper_diameter_mm / 2` to `bottom.lower_diameter_mm / 2` |
| foot | the top of the foot | `z = 0` | `bottom.platform_diameter_mm / 2` in to the bevelled rim |

The apex sits back along the tip's own axis, which is that normal, so the bite drives into
the face rather than up past it (ADR 0125, superseding that clause of ADR 0028). `facets`
sides are used for every ring; twelve puts the flats 0.026 mm inside the circle of a 2 mm
pillar, finer than a 9K pixel.

**Tips.** `cone` is an apex `bite_mm` up — smallest scar, weakest grip. `sphere` is a ball
with its upper pole at that apex — more material inside the model, so a firmer hold; its
rings run past the contact and rings never climb, so the neck is shorter than
`top.length_mm`. `plane` is the sweep's flat disc, for a face that must not be pierced.

**Feet.** `bottom.shape` sets the ring's side count and whether it tapers upward:
`cylinder` (`facets`, no), `cone` (`facets`, to 45% of the rim), `prism` (6),
`cube` (4), `skate` (no ring sweep at all). `platform_diameter_mm` is the circle the
corners sit on, so a cube's side is that over √2. The foot is a tube of its own, because a
tube has one side count, and the trunk runs down through it — the same overlap rule as
every joint (ADR 0041).

Every foot is bevelled at the plate: a round foot is one trapezoid from its rim in to
`rim - FOOT_BEVEL × height`, a skate's footprint is inset from its top, and the trunk stops
`FOOT_BITE` inside the foot rather than showing through the sole. A pad meeting the plate
on its widest edge is the pad a blade cannot get under (ADR 0044, ADR 0130). A tapering
foot keeps its rim as a middle ring, so it narrows towards the support and still meets the
plate on the bevel.

**The flare.** The pillar widens out into the pad over a cone that rises as far as it
widens, which is forty-five degrees. Its height is `(lower - upper) / 2` rather than a
field of its own, it is drawn with the pillar's facets rather than the pad's, and it sinks
`FOOT_BITE` into the pad the way the trunk does. It never reaches above the root it widens,
never narrower than the trunk standing in it, and never wider than the narrowest ring of
the pad under it (ADR 0130).

A skate is a closed box lying flat, elongated along a toe pointing away from the part — the
horizontal direction from the tree's highest tip to its root, or the plate's x if the tree
never leaned (ADR 0043).

**Small pillar.** `[small_pillar]` is the strut between two parts of a model: a diameter
and a depth per end, no foot. Which supports use it is derived — landing on the model,
carrying one tip, section enabled — never marked. A trunk keeps its width whatever it
stands on, because it carries the tips' weight (ADR 0042).

**Standing on the part.** Any other support that lands on the model ends in `[top]` and
`[tip]` upside down: a cone over `top.length_mm` from the trunk's own width down to
`top.upper_diameter_mm`, then the contact driven into the face. Leaving the trunk at its
own width rather than at `top.lower_diameter_mm` is what keeps a rim from hanging off a
merged trunk. What touches the part is a tip's scar whatever the trunk carries, so it
breaks off by hand (ADR 0124).

**Raft.** `[raft]` lays a slab from `z = 0` to `thickness_mm` over the feet that reach the
plate. It lifts nothing: supports still land at `z = 0` and pass through, and the fill rule
unions them (ADR 0043). The footprint is the convex hull of the foot centres (Andrew's
monotone chain) or their bounding rectangle, pushed out by a foot radius and spread about
its middle by √`area_ratio`, so the ratio names an area. `slope_deg` leans the walls in for
the same reason feet are bevelled.

**Bracing.** `[bracing]` ties trunks within `max_spacing_mm` every `rise_mm` from
`start_height_mm`, as high as both still stand — the lower of the two root nodes, since
above that a trunk has split. A tie is a cross, not a rung, so the pair is braced both
ways; a pair with room for one level is not tied. Each brace is one more tube, and one the
model is in the way of is not tied at all: a brace goes through ADR 0077's beam like every
other body, which is why `mesh_trees` is handed the model. The raft slab is the one thing
that is not beam-checked — it is a slab, not a body, and clipping it is a plate-level
operation.

**Lift.** `z_lift_mm` is how far the lowest point of a part stands off the plate, applied
by `core_geometry::lift_over_plate`; lifting twice is a no-op. The window applies it from
its **Lift models** button and the command line applies it inside `stand_under`, before it
cuts the model, leaving a part that already stands that high where it is. Whether the
lowest contact of a lifted part can stand at all is arithmetic: the pad, the top segment
and `MIN_PILLAR_HEIGHT_MM` have to fit under that contact, the segment being shortened
rather than allowed to eat the body's room. Light is sized to meet that at its own 2 mm —
a 0.5 mm pad, a 0.3 mm flare and a 1 mm top segment.

## Keeping the mesh closed

Rings are filtered before triangulation: a ring's Z is clamped to the one before it, so the
list never climbs, and a ring within `RING_EPSILON_MM` of the previous one is dropped. That
is what makes a column barely taller than its own foot safe — its neck collapses to nothing
rather than to zero-area faces.

Triangulation is then uniform: a fan from the apex to the first ring, two triangles per
facet between rings, a fan from the last ring to a centre vertex. Every edge lands in
exactly two faces for any ring count. Two rings at one height with different radii are
legal and describe the foot's shoulder. Tests assert closure through `diagnose` and the
volume against the closed-form cone, frustum, cylinder and disc.

## Picking one

`support_under` treats the strut below each tip as a capsule of the pillar's radius and
returns the tip whose axis passes nearest the camera along the ray. The tip is thinner and
the foot wider, but the strut under the tip is what the eye aims at and the one piece
belonging to a single support. A ray through a shared trunk hits nothing, since no single
support is there; alt-clicking a branch removes it and re-grows the tree.

## Placement

Generate asks what on this plate has nothing under it, by cutting the plate again and
reading the stack. Overhang angle is not the test: in resin what tears the film is area
that appeared from nowhere (ADR 0029).

```
Generate ─> SupportJob ─> slice each model ─> generate_supports ─> contacts ─> ObjectSupports::add
```

Each visible model is cut on its own at the Slicing section's layer height, so a contact
belongs to its model and travels with it (ADR 0028). The job reports progress and cancels
like the slicing job (ADR 0018). Supports already under the model are handed in as seeds:
they hold what is near them and are never moved, so a run only adds.

**The grid.** Layers are read onto 0.1 mm cells anchored to the plate by the same
`ScanlineRasterizer` the printed mask comes from, binary-shaded: a cell is material when
the layer covers its middle. The result is a `Field` — every row's spans in one buffer —
and every question is a merge walk over two fields' rows (ADR 0032). Contours are thinned
by half a cell from the ring's lowest point, so two layers of one wall thin identically and
cost does not follow the triangle count.

**What is held.** Both questions are asked of the cells this layer is the first to cover —
the layer less the one below — which is a layer's only new underside.

| On the layer | What it is | What is held |
|---|---|---|
| a new piece touching nothing below | an **island** | all of it, if at least `MIN_ISLAND_MM` (0.4 mm) across |
| new material past the reach from the layer a millimetre below | an **overhang** | the part past the reach, if at least `min_overhang_mm` across |
| new material past `min_overhang_mm` from the layer directly below | a **step** | the part past it, held as an overhang (ADR 0080) |
| a contact that would sit under a foot and the shortest pillar | standing on the plate | nothing |

The reach is `REFERENCE_RISE_MM * tan(max_overhang_deg)`, measured over that rise because
at 0.05 mm layers any angle worth allowing moves less than one cell per layer (ADR 0031);
near the bottom it is scaled to the rise available. `uncarried_by` never builds the grown
layer: it subtracts the reference rows as it reads them, widened per row, and abandons a
row at the first subtraction that empties it — which is every row of every wall.

A contact goes a whole layer below the plane that was cut, because the surface is somewhere
inside the layer's rise and a contact above it would land the column on the very surface it
holds. The tip then sinks `contact_depth_mm` back in.

**Where the points go.** A piece is sampled on a lattice of `spacing_mm` anchored to the
plate, every other row offset half a step; anchoring is what lets the coverage check thin a
slope to one support per place rather than one per layer. The rim is walked at the same
spacing and a rim point is dropped where a lattice point already holds the place — a
lattice leaves its widest gap exactly at the edge, which is where the peel starts. A piece
too small to catch a lattice point is all rim, so sampling never returns nothing.
`spacing_mm` is the profile's own `contact_spacing_mm` divided by the root of `density`, so
the knob counts supports over an area: doubling it puts down twice as many (ADR 0081), and
widening a contact holds better without thinning the lattice (ADR 0131). The head's carry —
a 0.4 mm head carries about 1.65 mm of island — is what `min_overhang_mm` is half of: a
ledge that small is inside the head that would be put under it.

**No doubling up.** A support holds a ball of `coverage_radius_mm` (¾ of the spacing)
around itself, so its reach falls off as the part climbs away: `sqrt(r² − rise²)`, nothing
past the radius (ADR 0079). A sample inside that of a support already standing — placed
lower in this run, or a seed — is skipped. A seed sits on the surface and a placed contact
a layer under it, so a support one layer above a place still counts.

The ball is asked of one part only. `Field::parts` labels the pieces of a layer and
`Parts::parents` maps each onto the pieces under it, so placement carries a set of standing
supports **per piece**: a piece inherits what stood under the pieces it grew out of, merged
where several do, and an island starts with nothing (ADR 0081). A column passing unrelated
material therefore holds none of it. Each set sits in buckets one radius across, so a
sample asks nine buckets, and a point more than a radius below the layer being looked at is
dropped as the run climbs — which is what keeps the run's memory to the supports that can
still hold something.

**Cost.** The stack is walked in blocks of 32: read and examined in parallel, since what a
layer needs depends only on the layers under it, with only the coverage check in order,
because that is what makes a run repeatable (ADR 0033). `cargo bench -p core-supports`
holds the numbers.

**The overhang wash.** With the tool in hand the viewport shades faces leaning past
`max_overhang_deg`, live. That is a face-angle test in the fragment shader and not what
placement measures — a face on the plate is marked though it needs nothing, a flat island
is marked for the wrong reason. It answers the question the slider raises (ADR 0034).

## Merging tips into trunks

A bed of parallel pillars costs resin, peel force and cleanup, so `grow` merges them by the
rule of *Clever Support* (ADR 0040).

Each tip owns a cone of descent directions bounded by `branching.max_angle_deg`. With `t`
the ground covered per millimetre of descent (the tangent of that angle), two fronts `p`
and `q` with `d` of ground between them meet at

```
from_p = clamp((d + (z_p - z_q) · t) / 2, 0, d)
z      = min(z_p - from_p / t, z_q - (d - from_p) / t)
```

— the midpoint shifted towards the lower front by the higher one's head start. The clamp is
the case where one front reaches the other's column outright.

Candidates sit in a max-heap on `z`, so the highest meeting point — the merge that saves
most — is taken first, and only when the two are within
`branching.max_merge_distance_mm`, neither strut is zero length, each strut's body clears
the model, and the meeting point can land. The trunk
takes `(r_a^k + r_b^k)^(1/k)` for `trunk_exponent` `k`, capped at `max_trunk_diameter_mm`
and never thinner than either branch: 2 conserves cross-section, 3 volume, the presets ship
2.5. A trunk is a front again; a candidate whose fronts have merged is dropped when popped.
Fronts sit in a grid of the merge distance, so a new one asks nine buckets. Survivors drop
a vertical trunk to their landing. `branching.enabled = false` gives back step 7a's column
exactly.

**Keeping out of the model.** Every body — each strut of a merge, the trunk under every
landing, and the foot on the plate — is swept as a beam of its own radius plus
`clearance_mm` and tested with nine rays: its axis and eight on the ring (ADR 0077). A first hit on a back face is a body that set off inside
the solid, which a forward ray otherwise reports as clear air out to the far wall. Where a
body belongs in the surface it is exempt: the head at a tip is trimmed off by
`top.length_mm`, and a landing stops the beam `(radius + clearance) · tan(slope)` above the
face, which is where the beam already reaches it. A face leaning more than 45 degrees from
horizontal is not a landing: there is nothing under a foot on it.

**When there is no room.** A tip whose own column is refused is not thrown away.
`columns` hands it over with no landing, merging gets its chance at it, and a front still
standing on nothing at harvest bends a **knee**: eight directions and three reaches,
shortest first, at the branch angle and no further than the merge distance or than the
height left under it allows (ADR 0078). Both the lean and the trunk under it go through
the same beam. What still cannot stand is dropped, and the panel's
standing-against-placed count is where that shows.

**Meshing.** Every strut is its own closed tube and tubes overlap rather than weld
(ADR 0041). A strut stops dead at its node; a ball `JOINT_SWELL` wider than the trunk,
centred there, hides every end cap whatever the angles and however many struts meet
(ADR 0044). A node one strut runs straight through — a tip square under the face it holds,
over a pillar going the same way — gets no ball and the strut above reaches
`JOINT_OVERLAP_MM` past it instead, so a plain column wears no bulge (ADR 0125). Ring
frames come from the tube's direction, chosen so a vertical tube uses the plate's own X
and Y.
`tests/branching_prints.rs` checks the point of all this: through a joint the layer has
several contours whose exposed area is strictly less than their sum, and under the joint it
is one trunk circle within 5%.

Merging is sequential, because the heap must drain in order for a run to be repeatable, and
at microseconds against the milliseconds placement costs it has not been worth splitting.
Volume figures read a little high, because an overlapping mesh counts its overlaps twice in
`signed_volume` (ADR 0041).

## Groups

A support carries the number of a group, and `Profiles` is the table of profiles the plate
is built with (ADR 0094). `columns` builds each tip to its own group's shape, `grow` runs
once per group — so a trunk is never shared by two shapes — and `mesh_trees` meshes each
tree to its group while the raft and the bracing come from group 0. A tree also carries the group it was grown to, so what meshes it knows its shape.

The table lives in the window's Supports tool: the panel's fields edit the group in hand,
and the scene stores only the number each support carries.

Dropping a group takes every support built to it away — a group is a shape, so its supports
do not survive it to be rebuilt to another group's numbers — and shifts the groups above it
down into its number. The window asks first when the group holds anything, and the history
takes the whole thing back. Group 0 cannot go: it holds the raft's and the bracing's
numbers.

## Editing a support

A support taken hold of is frozen: the tree as it stands is copied into the object in the
model's own space, and the points its tips grew from leave the automatic run (ADR 0095).
A rebuild then maps it back into plate coordinates and meshes it with the grown ones —
`ObjectSupports::trees` hands over the grown trees first and the frozen ones after them,
and `frozen_of` says which is which.

`grab` names what a ray met: `Node`, `Strut`, `Trunk` or `Foot`. Nodes and the foot are
aimed at as balls 1.6 pillars wide and win a tie with the stick they sit on. A press picks
a part and the press after that carries it, by the step the cursor takes on the plane
through it facing the camera; everything else picked takes the same step. A node bends
what meets there, a strut moves both its ends, and the foot stays on the plate.

Every step is then held to the rule the automatic run places by: `on_model` pulls each tip
back onto the nearest surface it holds, `fits` sweeps the whole tree as beams with the
profile's clearance — tip heads apart, since a tip is meant to touch — and a step that
sinks any part of it into the model or lands a tip on a blocked face is dropped, cursor
and support staying where they were.

## Painting a patch

A `Region` is one bit per face of the model. It is painted with a brush — a radius on the
plate, measured against each candidate face in the model's own space — or flooded from one
face across shared edges while the normal stays within an angle of the one that was
clicked. The face adjacency a flood walks is built for that click and thrown away.

Paint says where a support may go; `max_overhang_deg` says whether one is needed. `project`
drops every face of the region leaning further from a ceiling than that angle — the same
test the viewport washes an overhang with (ADR 0034) — so a brush that strayed onto a
vertical wall does not line it with supports, and the rim it walks is the rim of what is
left.

`project` turns the painted region into contacts in two halves, both optional:

- **Inside.** A grid of the plate at the asked-for spacing; over each cell the lowest
  surface of the patch is taken, so a patch that folds over itself is held from below.
- **Rim.** The edges the region ends on — the faces across them are outside it — walked at
  a spacing of its own. No polygon is offset (ADR 0092).

The patch's own geometry is kept apart from the columns and rebuilt once a frame while a
stroke is being drawn, because the columns take too long to rebuild per frame and a brush
that shows up only on release cannot be aimed.

The rim goes down first and every candidate passes one crowding rule: nothing within
three quarters of the finest spacing of anything already taken, the supports already
standing included. That is what keeps a rim of triangles shorter than the spacing from
becoming one support each, and a second Fill from stacking on the first.

Each contact then goes through `ObjectSupports::add` like a click, so a fill is undoable,
retunable and repeatable, and hand-placed supports survive it.

A second region is the blocker. It forbids a contact within a tip's radius of it, a fill
over its faces and any landing on them, so a support pushed off a painted face leans past
it rather than vanishing (ADR 0093). The four things a placement asks about — the mesh,
its hierarchy, its placement and this patch — travel together as `Placed`.

## In the window

Supports are drawn as a second mesh per object with an identity transform — they are
already in plate coordinates — in a cooler, darker colour; a painted patch is drawn the
same way, lifted 0.05 mm off the faces it covers, green for a fill and violet for a
blocker, which is a colour no overhang wash is.
While a brush is out, a press that lands on a model paints until the button comes up and a
press that misses one still turns the camera; under **Edit** a press picks the part it landed
on and draws it green, the press after that carries it, shift adds to what is picked, and
the columns are rebuilt in that same frame so nothing vanishes under the cursor. The stroke is painted along the line
the cursor drew rather than only where the frames fell, and the brush itself is drawn as a
ring lying on the surface under the cursor, in the colour of the patch it paints: drag to
paint, ctrl-click for the whole surface, alt to erase. The inspector's Supports section
carries the group picker with its name, New and Drop, the preset pill, a **Regular
Support** / **Small Pillar** tab pair, and below them
branching, raft, bracing, the lift and its button, a **Place** / **Paint** / **Block** pill
with the mode's own one-line hint under it, the brush, the surface angle, the two fill
spacings and Fill — and, under **Edit**,
how many parts of how many supports are held and the three buttons that move them to the
group in hand, grow them again or take them away — the overhang angle, the density,
Generate with its progress bar and cancel, the standing-against-placed count, and Save and
Load for the profile as TOML. Editing any field moves the pill to `Custom`; fields are
independent, so the order a profile must keep — every ring at least as wide as the one
above, the small pillar no wider than the pillar it replaces — is restored after an edit
rather than forbidden during one.
