# Supports

How a support gets from a click into the printed file. Why it is built this way:
ADR 0026–0029, 0031–0034, 0036, 0040–0044, 0092–0095, 0329.

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

A tip leaves the surface along its face's normal: `columns` reads the normal of the face
nearest the contact and puts the **neck** — the far end of the `[top]` segment — one
`top.length_mm` out along it, and shorter where a full one would eat the room the body needs
over the plate (ADR 0125). A normal more than 45 degrees off straight down is turned back to
45, by at most 33. Behind the tip's half ball a turned head runs at the contact's width for
`r · tan θ` before it widens, so a face cut slantwise meets only the ball and that pin: the
mark reaches `r / cos θ` on one side, 9 per cent wider at most. The pin is at most
`r · tan 33°`: 0.32 mm on Heavy's 1 mm contact, 0.19 mm on Medium's; only what is under the neck
leans and bends (ADR 0339). A contact off the
surface sends it straight down. A knee
and the strut into a thicker support are held to 45 degrees, whatever the branch
angle; a trunk is swept for clearance from its own width below its top; and a tip that can
neither stand nor lean is first let stand, or lean and stand, on a face up to 75 degrees
steep, and failing that run on one strut straight onto the model under it — onto a face
that looks up at it, so a contact inside the model is not run out through its skin. A contact on an edge or a corner sits on every face meeting there
and leaves the one that looks down most, so the support under a low point of the model is
shaped like the rest. Only the tip may meet the model: the whole head, contact to pillar
width, is swept from a hair past its pin, and a support whose head meets the model there,
or rises so nearly straight up that a trunk from its neck would come down on its own
contact, is not placed: `columns` leaves it out and the panel counts it with those that have
no room to stand (ADR 0339). First it is moved to the nearest spot of a face looking down
within 0.3 mm across and 0.05 mm up or down where a head has room, on rings 0.05 mm apart
every 30 degrees — out to its radius and a ring where that brings its whole circle onto its
face, which a contact with room moves for too (ADR 0339); failing that, in a profile with small pillars it has no head, and `grow`
holds it with a small pillar or not at all (ADR 0340). Everything below
is measured from the neck, not from the contact.

A ray runs down from `SURFACE_EPSILON_MM` (1 µm) below the neck — starting at the surface
itself would report the contact's own face at zero distance.

- Hit above `z = 0`: the column lands on the model and its lower end sinks
  `contact_depth_mm` into that surface, so the two cure as one solid. A profile with
  `land_on_model` off refuses that hit, and the tip bends a knee or is dropped (ADR 0124).
- Otherwise it lands on the plate and gets a foot.

Less than `MIN_PILLAR_HEIGHT_MM` (0.2 mm) of room means no column. The point is kept — the
model may move up — and the inspector reports standing against placed.

A front that cannot stand, nor lean within a merge's reach, searches further before it
is run onto the model: out to where a 25 mm strut at the profile's lean ends, every
1.5 mm across and 16 ways round, onto a gentle face and then a steeper one (ADR 0338).

## Checking a body against the model

Every placement question sweeps a `Beam` — a cone with its clearance — by the nine rays of
ADR 0077 and a row of balls down its axis, each as wide as the beam where it stands, no
wider than its distance to either end, half a radius apart; a ball is measured against
each face the `Bvh` offers, exactly, on the plate (ADR 0337). Once a family has grown, each
tree is held to its drawn bodies: `mesher::bodies` hands back the tubes the mesher sweeps,
and `sound` tests each ring-to-ring frustum, a 0.05 mm graze narrower, from where the
tube stands that graze off the face its tip — or the foot it stands on the part with —
was built to meet. A merged tree whose own branches fail stands those tips alone and
grows the rest again; one whose trunk or an inner strut fails is split at its lowest
joint and each half grown again (ADR 0337), and after three rounds what still fails is
stood tip by tip; a support of one
tip that fails is left unstood, and the inspector counts it. A dragged support is held to
the same test.

## The shape of a support

The profile is grouped as MSLA slicers group it, so numbers tuned elsewhere carry across:
`[tip]`, `[top]`, `[middle]`, `[bottom]`, `[small_pillar]`, `[branching]`, `[grid]`,
`[raft]`, `[bracing]`. An unmerged support is a solid of revolution about its own axis:

| Piece | From | To | Radius |
|---|---|---|---|
| top segment | a half ball of the contact's radius, `tip.contact_depth_mm` into the model (ADR 0339) | `top.length_mm` along the face normal, leaned back towards straight down by at most 33 degrees behind a pin (ADR 0339) | `tip.contact_diameter_mm / 2` to the pillar's radius |
| pillar | the top segment | the top of the flare | the node's radius, `middle.diameter_mm / 2` unless it is a trunk |
| flare | the pillar | the top of the foot | the pillar's radius to `bottom.lower_diameter_mm / 2` |
| foot | the top of the foot | `z = 0` | `bottom.platform_diameter_mm / 2` in to the bevelled rim |

The bite goes back along the tip's own axis, so it drives into
the face rather than up past it (ADR 0125, superseding that clause of ADR 0028), and no
deeper than half the wall behind the contact, measured by one ray along that axis: a tip
on a thin edge would otherwise come out of its far side as a nub, and above the model's
top as layers of nothing but tips. `facets`
sides are used for every ring; the default of 12, shaded smooth, reads as round, and a
joint's domes take six rings so a bend reads round (ADR 0341).

**Tips.** `plane`, the default, ends in a half ball of the contact's radius centred
`bite_mm` into the face along the head's axis, the cone running from its equator: at a
bite of nought half the ball is in the part, and the face cuts it in a circle of the
contact's width however the head leans. Its pole is held to half the wall, the centre
coming back out where the ball would reach further (ADR 0339). The tip a trunk standing on
the part ends in is rounded the same way, and so is each end of a small pillar, to its own
diameter whatever the tip shape (ADR 0340). `sphere` is a
ball with its upper pole `bite_mm` in — more material inside the model, so a firmer hold;
its rings run past the contact and rings never climb, so the neck is shorter than
`top.length_mm`.

**Feet.** `bottom.shape` sets the ring's side count and whether it tapers upward:
`cylinder` (`facets`, no), `cone` (`facets`, to 45% of the rim), `prism` (6),
`cube` (4), `skate` (no ring sweep at all). `platform_diameter_mm` is the circle the
corners sit on, so a cube's side is that over √2. Every pad is that wide, and neighbouring
pads overlap into one where their supports stand closer than it. A pad the model is in the
way of is moved aside whole — `Landing::pad_offset`, tried in eight directions as far as
still leaves the flare inside its sole. Where that is not room enough the pad is cut
back instead, `Landing::pad_cut`: the nearest model along each of 16 sectors' edges and
middle, at its sole, middle and top, less `clearance_mm`, as long as no sector bares the
flare; each look stops at the pad's reach, and one ray per height says whether the pad sets
off inside the model (ADR 0327). The mesher pulls the foot's vertices in to it. Failing that the support has no
landing there and leans for one like any other blocked front (ADR 0239, 0327). The foot is a tube of its own, because a
tube has one side count, and the trunk runs down through it — the same overlap rule as
every joint (ADR 0041).

Every foot is bevelled at the plate: a round foot is one trapezoid from its rim in to
`rim - FOOT_BEVEL × height`, a skate's footprint is inset from its top, and the trunk stops
`FOOT_BITE` inside the foot rather than showing through the sole. A pad meeting the plate
on its widest edge is the pad a blade cannot get under (ADR 0044, ADR 0130). A tapering
foot keeps its rim as a middle ring, so it narrows towards the support and still meets the
plate on the bevel.

**The flare.** The pillar widens out into the pad over a quarter-circle fillet, tangent
to the pillar above and the pad below, that rises as far as it widens (ADR 0341). It leaves the pillar at the pillar's own width, so
its height is `(lower - pillar) / 2` rather than a field of its own (ADR 0238), it is drawn with the pillar's facets rather than the pad's, and it sinks
`FOOT_BITE` into the pad the way the trunk does. It never reaches above the root it widens,
never narrower than the trunk standing in it, and never wider than the narrowest ring of
the pad under it (ADR 0130).

A skate is a closed box lying flat, elongated along a toe pointing away from the part — the
horizontal direction from the tree's highest tip to its root, or the plate's x if the tree
never leaned (ADR 0043).

**Small pillar.** `[small_pillar]` is the strut between two parts of a model: a diameter
and a depth per end, no foot and no tip — one width from the face it holds to the one it
stands on, each end a half ball of that width sunk by its depth, whatever the profile's tip
is. An end meeting its face more than 20 degrees off the normal first leaves it along the
normal for one diameter and bends there, so a face meets it in a circle of the diameter, at
most 6 % longer one way (ADR 0340). Which supports use it is derived —
landing on the model, carrying one tip, running no further than `max_length_mm` nor 30
diameters from contact to landing, section enabled — never marked; a longer one keeps the
regular width (ADR 0255, 0340). A trunk keeps its width whatever it
stands on, because it carries the tips' weight (ADR 0042).

**Standing on the part.** Any other support that lands on the model ends in `[top]` and
`[tip]` upside down: the trunk keeps its joint's width, then a cone over `top.length_mm`
down to the contact's, pushed its bite in and flat for a plane tip, or meeting the surface
and biting to a point for a sphere. What touches the part
is a tip's scar whatever the trunk carries, so it breaks off by hand (ADR 0124, 0242).

**Raft.** `[raft]` lays its pieces on the plate. A support that reaches the plate has no
pad then: its flare stands on the raft's top and bites into it, and the fill rule unions
them (ADR 0043, 0266). `style` picks the pieces: `skates`, a
slab of `thickness_mm` whose sloped wall carries on up into a lip as tall again, its
inner face at the same slope, like a saucer; a `grid` of bars inside a rim with a pad under each foot; or `connected` pads
joined by strips along the feet's Euclidean minimum spanning tree. A skates or grid raft
is drawn to the convex hull of the model's footprint and every flare's foot (Andrew's monotone
chain), pushed out `margin_mm` with rounded corners, or the smallest-area rectangle around
that hull pushed out as far (ADR 0241). A corner is rounded to at least twice the depth
the rim cuts in; under a smaller margin the hull is drawn in by the difference first, so
a straight side still stands `margin_mm` out. `slope_deg` leans the walls in for the same reason
feet are bevelled.

**Bracing.** Which trunks are tied is a graph: the Gabriel graph of the trunks tall enough
to carry one level, cut at `max_spacing_mm` — two are neighbours when no third stands
inside the circle they span as a diameter. It is planar, so no two crosses cross seen from
above, and it keeps every trunk's nearest neighbour. A tall trunk with nobody inside that
reach is leant towards its nearest neighbour on a knee, to three quarters of the reach,
where branching is on and the trunk under the knee can still be tied (ADR 0240) — but no
further across than half the merge distance, and towards a trunk of any group already
stood (ADR 0250). A brace ends on each trunk's own axis at that height, so one a hand has
leant keeps its braces.
`[bracing]` is each group's own (ADR 0252): a trunk is tied only where its group braces,
two groups are tied where both do and no further apart than both span, and the brace
between them is built to the finer of the two and meshed with that group. It ties those
neighbours one level every `rise_mm` from `start_height_mm`, as high as both still stand —
the lower of the two trunk tops: the root node, above which a trunk has split, or under the
head of a column holding its one tip. The levels fall into
zones (ADR 0328): `bottom` takes its levels up from the start, `top` its levels down from
that ceiling, and `middle` the whole rises between; a zone short of room gives way, the
bottom last. A level is a cross or a zigzag's one leaning brace, never a rung; a pair
without room for one level is not tied. A trunk left free above its highest brace by more than
`buttress.pair_above_mm`, or above its second-highest by more than `one_above_mm`, is
propped by two legs or one (ADR 0328): each leans 45 degrees out from the trunk's top for one
rise, away from the middle of the part, and stands on the plate. Legs are made from the
trees each time they are meshed, then braced like any trunk. A brace is no wider than either trunk
where it meets it (ADR 0242). Each brace is one more tube, and one the
model is in the way of is not tied at all: a brace goes through ADR 0077's beam like every
other body, which is why `mesh_trees` is handed the model. The raft slab is the one thing
that is not beam-checked — it is a slab, not a body, and clipping it is a plate-level
operation.

**Lift.** `z_lift_mm` is how far the lowest point of a part stands off the plate, applied
by `core_geometry::lift_over_plate`; lifting twice is a no-op. A raft stands the part up
by its own thickness on top of that, so the slab is never printed into it (ADR 0238): the
command line adds the two, and the window does it without being asked — every supported
model rises by the raft's thickness when the raft is switched on and comes back down when
it or the supports go, and what it rose by is kept beside its supports and in the project. The window applies it
per model from **Lift off the plate**, in Position and in Supports alike, or as auto-orient
stands a pose on supports, and the command line applies it inside `stand_under`, before it
cuts the model, leaving a part that already stands that high where it is. Whether the
lowest contact of a lifted part can stand at all is arithmetic: the pad, the top segment
and `MIN_PILLAR_HEIGHT_MM` have to fit under that contact, the segment being shortened
rather than allowed to eat the body's room. Every shipped preset lifts by at least that
much: all three lift 5 mm (ADR 0237, 0323).

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

**What prints.** A model is cut, landed and swept as it prints: drilled where its holes
have been taken out exactly, else as its shell once hollowed (ADR 0332). The cavity's
ceiling and the crown of a hole are then undersides like any other, and the faces under
them faces a column lands on. The patches stay the model's: a fill reads its faces, and a
blocked patch is asked by point, within 0.01 mm of it, so it holds on a surface numbered
otherwise. A contact whose nearest face bounds the cavity, or the lattice in it, is inside:
**Grow the supports** keeps only those outside, and **Support the cavity** keeps only those
inside; once something stands inside it is **Delete the cavity supports**.

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
| a new piece touching nothing below | an **island** | all of it, if at least `MIN_ISLAND_MM` (0.4 mm) across; a narrower one is a **speck** |
| new material past the reach from the layer a millimetre below | an **overhang** | the part past the reach, if at least `min_overhang_mm` across |
| new material past `min_overhang_mm` from the layer directly below | a **step** | the part past it, held as an overhang (ADR 0080) |
| a piece with no support and no plate under it or under anything it grew out of | **adrift** | the point it began at, once it is more than dust; else all of it, once it is `MIN_ISLAND_MM` across |
| a contact that would sit under a foot and the shortest pillar | standing on the plate | nothing |

The reach is `REFERENCE_RISE_MM * tan(max_overhang_deg)`, measured over that rise because
at 0.05 mm layers any angle worth allowing moves less than one cell per layer (ADR 0031);
near the bottom it is scaled to the rise available. `uncarried_by` never builds the grown
layer: it subtracts the reference rows as it reads them, widened per row, and abandons a
row at the first subtraction that empties it — which is every row of every wall.

The first three are read off a layer and the layers under it; adrift is read off the climb.
A point hanging down begins as a speck under `MIN_ISLAND_MM` and then widens inside the
angle, so no layer of it is an island or an overhang, and placement follows each piece's
parentage to know that nothing ever held it. A piece the blocker refused counts as held.

Where such a piece began at a **low point** of the model — a vertex lower than every
vertex it shares a face with, which `low_points` reads off the mesh — or as a speck, its
support goes to that point, or to the speck's middle, rather than to wherever its first
layers were wide enough to read. It goes down once what has grown out of the point is
more than dust: `MICRO_ISLAND_MM3` (0.01 mm³) of resin and `MICRO_ISLAND_HEIGHT_MM`
(0.08 mm) of rise, so a needle too thin ever to be an island is held at its tip, and a
speck that never grows prints loose. The resin is counted per piece while nothing holds
it, from one walk over the layer's spans, and summed where such pieces join. A low point
no adrift piece begins at within a millimetre of rise is a bump on a surface something
already holds, and gets nothing.

The grid is too coarse for a lobe a few tenths across, so once the climb is done every
layer is read again on 0.02 mm cells, and every piece touching nothing under it — dust
under 0.005 mm² aside, which the layer above does not stand on either — must have a
contact on its seat, 0.06 mm in from its rim, within two layers under its plane. One
without is held at the lowest low point on its seat, else the seat's cell nearest its
middle: a placed contact on its rim is moved there, else the nearest within 0.3 mm, else
one is added (ADR 0329). So a speck that never grows prints loose
only if it is too small for the check to name.

A contact goes a whole layer below the plane that was cut, because the surface is somewhere
inside the layer's rise and a contact above it would land the column on the very surface it
holds. The tip then sinks `contact_depth_mm` back in. A tip with no bite would never reach
the surface from there, so building its column moves the contact up onto the face over it,
when that face looks down and is no more than 0.5 mm above (ADR 0323). A contact inside the
model — a steadying support on the outline of a part's section — is moved down onto the
underside its ray leaves through, whatever the bite (ADR 0329).

**Where the points go.** The outline first, because a corner is where a ledge lifts and the
edge is where the peel starts. Corners are the vertices of the piece's convex hull where it
turns 45 degrees or more between a quarter of a spacing back and a quarter on — a piece is
cells, not a polygon, and a hull has a direction to turn from; a ring or a round rim has
none. The rest of the rim is then taken wherever nothing stands within a spacing, so no
rim cell is further than that from a support. The inside is filled from a lattice of
`spacing_mm` anchored to the plate, every other row offset half a step, and then from a
grid of a quarter of that into the gaps the rim's supports opened, both keeping three
quarters of a spacing from what is taken; anchoring is what lets the coverage check thin a
slope to one support per place rather than one per layer. A piece too small for two
supports gets one at its middle, where a corner would leave it a lever, so sampling never
returns nothing (ADR 0329). An island is sampled closer
than an overhang, by its area, as the profile's `[islands]` says: as shipped, under 2.4 mm²
at 13/18 of the spacing, under 3.2 mm² at 15/18, and at the spacing itself above that, each
keeping three quarters of its own spacing clear. Automatic sizing takes the two areas from
its own `island_area_mm2`, so one number picks both a support's size and its spacing. It hangs on the film alone, with no edge of a part to steady it.

**Clear of the edge.** A head centred on the layer's outline is half off the surface it
holds: its nearest face is the wall above the edge, so the tip leans out and cures onto
that wall. Every sample is therefore taken `edge_inset_mm` in from the outline of the
*layer* — the head's radius and one cell — which leaves the side a ledge is carried along
alone, since material lies on both sides of it. Where a piece is further than twice the
inset from any such room, it is narrower than a head and is sampled on its own cells as
before; a contact that cannot stand clear is still better than none.

**Three to stand on.** A part held at one or two places is held and still rocks on the
peel. Placement counts the supports under each piece and what it grew out of, and once
such a part is a spacing across it gets up to three more round its rim, 0.8 of its width
apart, whatever the coverage ball says. A part on the plate, one the hand has placed a
support under and one the blocker refused are left as they are.
**By the angle.** Under an overhang the spacing runs from `contact_spacing_mm` under a face
looking straight down to `steep_contact_spacing_mm` under one leaning just past the angle,
linear in the angle between the face's normal and down (ADR 0324). Each sampled cell is put
in one of `LEAN_BANDS` (4) bands by how far it lies past the reference layer — the band
edges are the leans a quarter, a half and three quarters of the way from the angle to level,
`Field::reaches` asked of that one cell — and takes its band's flattest spacing; a cell
more than a step past the layer directly under it is a shelf and level. The lattice stays at
`contact_spacing_mm`: the support placed there holds a ball of its own spacing less a
quarter of `contact_spacing_mm`, which under a level face only keeps the next layer off the
place and under a steep one is what spaces the supports.

`spacing_mm` is the profile's own `contact_spacing_mm` divided by the root of `density`, so
the knob counts supports over an area: doubling it puts down twice as many (ADR 0081), and
widening a contact holds better without thinning the lattice (ADR 0131). The head's carry —
a 0.4 mm head carries about 1.65 mm of island — is what `min_overhang_mm` is half of: a
ledge that small is inside the head that would be put under it.

**No doubling up.** A support holds a ball around itself — `coverage_radius_mm` (¾ of the
spacing) under a level face, its own as above elsewhere — so its reach falls off as the part climbs away: `sqrt(r² − rise²)`, nothing
past the radius (ADR 0079). A sample inside that of a support already standing — placed
lower in this run, or a seed — is skipped. A seed sits on the surface and a placed contact
a layer under it, so a support one layer above a place still counts. A seed the climb
reaches before any piece stands at its cell — which is every contact an earlier run
placed, a layer under the surface it holds — waits for that piece for as long as it could
still hold it, so a second run, in any group, adds nothing where the first already holds.

The ball is asked of one part only. `Field::parts` labels the pieces of a layer and
`Parts::parents` maps each onto the pieces under it, so placement carries a set of standing
supports **per piece**: a piece inherits what stood under the pieces it grew out of, merged
where several do and each support kept once however many of them bring it back, and an island starts with nothing (ADR 0081). A column passing unrelated
material therefore holds none of it. Each set sits in buckets as wide as the widest ball, so a
sample asks nine buckets, and a point more than its own radius below the layer being looked at is
dropped as the run climbs — which is what keeps the run's memory to the supports that can
still hold something.

**What each contact holds.** Every contact comes back with its `Reason`, which an
automatic profile sizes it by (ADR 0325): an island with its area on the layer it begins
on — a part that begins at a point has none — an overhang with the model above its layer,
or a support that steadies a part. The model above is known only at the top of the stack,
so the climb keeps a graph of pieces beside it (`load.rs`): a piece that is the only child
of its only parent stays on its parent's node, any other starts a node whose parents are
the nodes of what it grew out of. A node is made after its parents, so one walk down the
list adds each node's total into its parents, split evenly where it has several so a
joined piece is not counted twice. A contact remembers its node and how much of it lay at
or under its layer; the model above it is the node's total less that.

**Steps, when sized.** A run handed an automatic profile asks `unsupported` for the shelf
put out over the layer below apart from the lean over the rise, and for the faces leaning
past the profile's `step_lean_deg` and short of its overhang angle (ADR 0326). A step whose
piece puts out at least the least step volume is held as one and sized by that volume; its
samples become balls of the step's height above it in `step.rs`, bucketed by the tallest
height and dropped once the climb passes them. An overhang or a gentle face inside a ball
is held as a step of the largest one reaching it. A gentle face outside every ball, and a
slope past `steady_lean_deg` across at least the side of `steady_area_mm2`, is steadied on:
sampled at `steady_spacing_mm`, last, each one kept in a second set of balls that keeps
only its own kind away, so an overhang above it is still held (ADR 0326).

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
`branching.max_merge_distance_mm`, together carry no more than `max_tips_per_trunk` tips,
neither strut is longer than `max_strut_length_mm`, each strut's body clears the model,
and the meeting point can land. Where the higher front reaches the lower one's column at
or above it, the meeting point is the lower front itself and only the higher one leans:
it comes down onto that neck or joint, so a tip joins the trunk beside it rather than one
at its own height further off (ADR 0338). A pair refused for its bodies or its
landing is offered again with both fronts dropped `DROP_STEP_MM` (1 mm) straight down
first, then twice as far each time, until the meeting point reaches the plate: what blocks a strut is
mostly the part right beside the tips, and dropping past it is how two columns under a
crowded part still share a trunk (ADR 0338). The joint
keeps the branches' own width; only the trunk under the root widens, as a cone from that
width to a base of `min(cap, max(pillar, height / 15 / 2, carried))`, where `carried` folds
`(r_a^k + r_b^k)^(1/k)` for `trunk_exponent` `k` over the tips it holds: 2 conserves
cross-section, 3 volume, the presets ship 2.5 (ADR 0242). A trunk is a front again; a candidate whose fronts have merged is dropped when popped.
Fronts sit in a grid of the merge distance, so a new one asks nine buckets. Survivors drop
a vertical trunk to their landing. A merged front that stands on the model is split again
where every tip under it stands on the model as a small pillar of its own — from its neck,
or straight from its contact — since two thin struts snap off where a joint between two
faces has to be cut away (ADR 0340). `branching.enabled = false` gives back step 7a's column exactly.

**On a grid.** Where every group of a family has `[grid]` on, nothing is merged: each tip
goes to a node of a square grid `spacing_mm` across anchored to the plate (ADR 0333) — the
nearest of the 25 nodes round it whose strut from the neck, leaning 45 degrees, clears the
model, and under which a trunk lands straight down. The tips of a node hang one above
another on one vertical trunk, each from a joint where its strut meets it; the lowest joint
is the front that stands the trunk. A tip no node takes is stood as without the grid, and
no gridded trunk is leant towards a neighbour.

**Keeping out of the model.** Every body — each strut of a merge, the trunk under every
landing — is swept as a beam of its own radius plus `clearance_mm` and tested with nine
rays: its axis and eight on the ring (ADR 0077). A foot on the plate is tested from its
middle out instead, 48 rays across the plate at three heights, so a toe standing inside
it is found too (ADR 0327). A first hit on a back face is a body that set off inside
the solid, which a forward ray otherwise reports as clear air out to the far wall. Where a
body belongs in the surface it is exempt: the head at a tip is swept on its own (ADR
0339), the top of a trunk under it bare, and a landing stops the beam `(radius + clearance) · tan(slope)` above the
face, which is where the beam already reaches it. A face leaning more than 45 degrees from
horizontal is not a landing: there is nothing under a foot on it.

**When there is no room.** A tip whose own column is refused is not thrown away.
`columns` hands it over with no landing, merging gets its chance at it, and a front still
standing on nothing at harvest bends a **knee**: eight directions and a reach every
`ESCAPE_STEP_MM` (1 mm), shortest first, at the branch angle and no further than the merge
distance or than the height left under it allows (ADR 0078, 0338). A contact with no head
is not bent at all: it is held by a small pillar or by nothing (ADR 0340). From a diameter
out along its normal, the pillar casts 50 rays — along the normal, straight down, and 16
ways round at 15, 30 and 45 degrees off straight down — for a face of the part looking back at it, no steeper than 75 degrees,
and of the sound runs whose struts lean no more than 45 degrees takes the one that turns
least at its joints, then the shortest. The same search
is the last resort of any tip that cannot stand, before a strut straight down, and of a tip
whose support stays unsound. Both the lean and the trunk under it go through
the same beam. What still cannot stand is dropped, and the panel's
standing-against-placed count is where that shows.

**Meshing.** Every strut is its own closed tube and tubes overlap rather than weld
(ADR 0041). A strut keeps its own width all the way down to the node; the tube leaving the
node starts under a dome of its own. A trunk is never thinner than a branch, so every end lies
inside what it meets: nothing covers the joint and nothing is wider there than the trunk
itself (ADR 0238). A strut ends flat where that tube runs on twice the strut's width, and in a
dome of its own width elsewhere (ADR 0341). Ring
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
once per family of groups — the groups that branch and whose pillars are one width — and
`mesh_trees` meshes each tree to its group, each head to the group it names and each brace
to the group it ties. The raft is not a group's: it is the
model's own or the plate's (ADR 0253), written into every profile of the table a model is
built with; the core builds the slab from the first and stands every group on it (ADR 0266). A tree also carries the group it was grown to, so what meshes it knows its shape.

Within a family every front carries the group of the heaviest head under it — the larger
contact, then the wider pillar, then the lower number — and two fronts of two groups meet
within both groups' branch angles and merge distances, clear by the larger clearance, and
stand by the heavier one's profile (ADR 0325). A tree is stamped with its root front's
group; a tip of another names its own in `TreeNode::head`, so a lighter head merged into a
heavier trunk is still built as its own group's. `mesh_groups` puts each strut in the
mesh of the heaviest head hanging from it, and the trunk and foot in the tree's.

Families are grown thickest first, and the struts and trunks a family has stood are
carriers for the families after it (ADR 0248): while those merge, a front may run into a
carrier at least 0.05 mm wider than itself — by one strut within its own branch angle and
half its merge distance, at the highest point it reaches — in the same heap its merges
with other fronts sit in.
Such a front is a tree whose `Landing` is that point with `on_support` beside `on_model`,
so it has no trunk, no foot, no pad and no brace, and the mesher ends it plain inside the
carrier. A tree built as a small pillar carries only as wide as that, and its trunk
carries nothing, since it may yet be moved from under what stood on it; once every group
is grown each one with a neck is itself run into a wider carrier in its reach, by a strut
no longer than its own drop to the part, and built that thin all the way up, so a small
pillar stands on a support and never the other way round (ADR 0251, 0340). Last, every small
pillar on the part is replaced by what that search finds from its contact where that turns
five degrees less, and squared onto both its faces where that is sound and leans no strut
past 45 degrees (ADR 0340). `grow` hands the forest back in group order.

Any group can be dropped but the last, and the one after the first takes its place and the
raft with it. A secondary click on a group's row builds the whole group to another saved
profile (`SupportTool::retune_group`).

A group can be left out of the view by the eye on its row. It is still printed; its
supports are neither drawn nor aimed at.

The table lives in the window's Supports tool, and the scene stores the number each
support carries. A model may keep a profile of its own in place of a group's
(`OwnSettings::supports`, ADR 0246): the panel's fields edit the picked models' and the
plate's only with nothing picked, `SupportTool::table_for` is the table a model is built
to, and saving a profile is what hands an edit on to every other model built to it.

The preset list at the top of the Supports card chooses a group, not numbers: the group
built to the chosen preset becomes the one in hand, one is made for it when there is none,
and what stands keeps the group it stands in (ADR 0247). A group nothing stands in is
retuned instead. So two kinds of support on one part are two choices from one list, and a
support changes kind by being picked and moved to the group in hand. The list shows only
the groups that hold supports, each under its preset's name, on the line of that preset
(ADR 0336).

Choosing **Auto** in that list sizes supports automatically, any other preset by hand.
Under Auto the list shows the shapes it built; in the Shape mode it shows every shape it
builds to, since each is edited there like any support profile. Sizing automatically, the
card's placement and size fields edit the automatic profile's own numbers, which Save
writes over the shipped one (ADR 0325), and a run is placed by them on its lightest shape;
each contact then goes into the group built to the shape its `Reason` sizes it into — a
group of that profile's name, else one nothing stands in, retuned, else a new one — and a
shape nothing is sized into gets no group (ADR 0325).

Dropping a group takes every support built to it away — a group is a shape, so its supports
do not survive it to be rebuilt to another group's numbers — and shifts the groups above it
down into its number. Its bin asks twice rather than a dialog (ADR 0336), and the history
takes the whole thing back. Group 0 cannot go: it holds the raft's and the bracing's
numbers.

## Editing a support

A support taken hold of is frozen: the tree as it stands is copied into the object in the
model's own space, and the points its tips grew from leave the automatic run (ADR 0095).
A rebuild then maps it back into plate coordinates and meshes it with the grown ones —
`ObjectSupports::trees` hands over the grown trees first and the frozen ones after them,
and `frozen_of` says which is which.

Growing is the costly half of a rebuild — about a second for fifteen hundred heavy tips —
and meshing the cheap one, so the grown trees are kept apart from the meshes and grown again
only when the points, the placement, the table or the blocked patch change. Freezing a
support lifts its tree out of them and renumbers the rest, and carrying, regrouping or
cutting a frozen one only meshes again. A table changed while a field is still dragged is
grown to once the pointer is let go.

`grab` names what a ray met: `Node`, `Strut`, `Trunk` or `Foot`. Nodes and the foot are
aimed at as balls 1.6 pillars wide and win a tie with the stick they sit on. A ray that
goes through nothing takes the part it passes nearest, within a slack the viewport sets to
two points of the screen where the camera is looking: a pillar seen from far away is a
point wide, and a part the ray does go through always wins over one it only passes.
The part a press would take is drawn fainter than what is held before the button goes down.
A part met further along the ray than the first printed surface is behind that model and is
not taken, hover or press: `grab_along` answers how far along the part stands. The ray is
measured from where the section lets it in, and a cut standing in material is a cap that
hides what is behind it, so a part inside a cavity the section opens is taken (ADR 0332).

A press picks a part and the same drag carries it, by the step the cursor takes on the
plane through it facing the camera, measured from where the cursor was pressed and not from
the part's middle; everything else picked takes the same step. A step the model refuses
whole is tried again as its run across the plate and then as its rise, so a part slides
along what it touches instead of stopping.

A tip held alone is not slid by a step but aimed, like a new support: `tip_carried` puts
it where the cursor's ray meets its model, or on the surface nearest the cursor at the
tip's own depth when the ray misses or that place is refused. Its neck goes with it, one
`[top]` out along the face it now holds, and a support that is one tip on one trunk drops
its trunk from the neck again, so the whole of it follows the cursor.

**Remove these supports** takes away what is picked and everything it holds up
(`SupportTree::without`): a strut or a joint goes with the tips above it, a joint left
holding nothing goes after it, and the support goes whole when no tip is left or its trunk
or foot was picked. **Remove all** takes every support off the models aimed at, the
ones a hand froze included, and **Only** *group* those of the group in hand alone,
leaving the group and every other group's number as they were. Shift only
adds the part to what is picked or takes it out. The gesture is the support's from the
press to the release, wherever the cursor goes, so the camera neither turns nor slides
under it. A node bends what meets there, a strut moves both its ends, and the foot stays on
the plate.

Every step is then held to the rule the automatic run places by, which is what `carried`
answers with: `on_model` pulls each tip back onto the nearest surface it holds, `fits`
sweeps the whole tree as beams with the profile's clearance — tip heads apart, since a tip
is meant to touch, and a trunk standing on the model swept only down to where its rim may
meet the face it sinks into — and a step that sinks any part of it into the model or lands a tip on a
blocked face is dropped, cursor and support staying where they were. Which nodes a `Part`
moves, and that the foot stays on the plate, are settled there too and not in the panel
that reads the drag (ADR 0208).

## A line of supports

Shift-click with Place in hand stands a row from the last support placed on the same model
to the click (ADR 0330). `inline` cuts the chord between them into the fewest equal steps
no longer than the line spacing and takes each step to the nearest surface, the way a
carried tip is pulled back onto its model; a point on a blocked face or within half a
spacing of the one before is dropped, and each one left goes through `stands` like a click.

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
  Or rings following the rim, a spacing apart along and across them, or a scatter no
  closer than the spacing, ordered by a hash of each cell so it repeats.
- **Rim.** At no offset, the edges the region ends on — the faces across them are outside
  it — walked at a spacing of its own (ADR 0092). Otherwise a ring that far in, and a
  second ring its gap further in if one is asked for.

Anything measured in from the outline is read off the **footprint** (ADR 0331): the lowest
surface over cells a quarter of the finest spacing across, 0.1 to 0.5 mm, and each cell's
distance to the nearest cell the patch is not over, by the two-pass 8SSEDT sweep. A ring at
depth `d` is the band of cells in `[d - cell/2, d + cell/2)`, walked from its first cell
in row order to the nearest unvisited one within two cells and kept wherever it has gone
a spacing from the last point kept, one loop after another. The inside starts its own
offset in from the innermost rim, or from the outline with no rim. Depth is measured
across the plate, so on a steep patch an offset is shorter along the surface than asked.

`project` hands the rim back apart from the inside, and the window builds each to the
saved profile chosen for it — the group built to that profile, else one made for it — or
to the group in hand.

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

The trees are grown and meshed on a worker, against the clearance answers the model kept
from its last growth; until they land the supports drawn last stay, and slicing and the
preview wait (ADR 0342).
Supports are drawn as a second mesh per object with an identity transform — they are
already in plate coordinates — in a cooler, darker colour; a painted patch is drawn the
same way, lifted 0.05 mm off the faces it covers, green for a fill and violet for a
blocker, which is a colour no overhang wash is.
While a brush is out, a press that lands on a model paints until the button comes up and a
press that misses one still turns the camera; under **Edit** a press picks the part it landed
on, draws it green and carries it (ADR 0244), and
the columns are rebuilt in that same frame so nothing vanishes under the cursor. While **Place** is in hand,
the support a click would stand is drawn under the resting cursor, see-through in its group's tint, as
`core_supports::preview` grows it alone — what it would merge with or be braced to is left
out — or only its tip, in red, where nothing would be built; a click there puts nothing down.
The Hollow and Drain tools show their blocker, hole or channel point the same way, see
`docs/design/viewport.md`. The stroke is painted along the line
the cursor drew rather than only where the frames fell, and the brush itself is drawn as a
ring lying on the surface under the cursor, in the colour of the patch it paints: drag to
paint, Cmd-Shift-click for the whole surface, alt to erase. The Supports card's modes and blocks are
ADR 0336's. Editing any field marks the preset edited; fields are
independent, so the order a profile must keep — every ring at least as wide as the one
above, the small pillar no wider than the pillar it replaces — is restored after an edit
rather than forbidden during one.

**The Shape mode's drawing.** Each part is drawn from the same measurements the mesher
builds it to — the plane or sphere tip, the trunk's cone up to the cap, the flare, the
foot's shape and bevel, the raft's style — at one scale for lengths and widths alike, so a
proportion on the panel is the proportion on the plate. Each view zooms to fit its own
part: the tip close up against the model; two tips the merge distance apart meeting at the
branch angle and the trunk down to the plate, its long middle cut out behind a break; the
foot, on a support as short as the lift allows, and the raft's rim
standing on the plate, since they are wide and thin; two trunks the spacing apart with
their crossed braces. What runs past the frame is clipped, not shrunk.
