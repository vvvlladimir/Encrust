# Orientation and arranging

Which way up a model prints best, and where several of them go: what `core-plate` decides.

## What is being minimised

An MSLA print fails on the peel before it fails on anything else: every layer is pulled
off the film, and that force follows the cross-section being pulled. So the score is not
the support-volume score an FDM tool uses. Each term is made dimensionless, so that a
miniature and a bust are scored on the same scale:

| Term | What it is |
|---|---|
| support | overhang, tips and support height, over the surface area (below) |
| volume | the support resin a pose is predicted to need, over the surface area times the length (below) |
| peel | the area of the disc pulling as hard as the hardest section, over the model's own square length |
| height | how tall the stack stands, the model and the lift under it, over the model's own length |
| bottom | area flat on the plate, over the surface area; zero on supports |

The support term adds three things a support is put under:

- the downward area past the overhang angle, weighted by how far past;
- a tip, lower than every vertex it shares an edge with: each one is charged as three
  flat patches one contact holds, `contact_spacing_mm` squared, since the model can hang
  off it;
- half again the overhang for support standing the model's own length tall, so a high
  ceiling costs more than a low one.

The volume term is what the fewest supports reads (ADR 0279): every face past the overhang
angle, however far past, and within a degree of it, since placement reads the angle off
pixel steps and holds much of a face that meets it exactly, by its plan area — placement spreads a contact a spacing apart
over all of it — times the height it is held at, plus the lift under all of it, plus each
tip standing half the model's height up.

A section's pull is the one `core-analysis` reads off a cured layer, `pull_n`
(`docs/design/analysis.md`). Scoring the area of the disc that pulls as hard keeps the term
in the units of the shadow it replaces. A disc pulls the most an area can, so a thin strip
or two pieces apart score less than their area, and a shadow scored as a disc never less
than a cut.

| Preset | support | volume | peel | height | bottom |
|---|---|---|---|---|---|
| Fewest supports | — | 1.0 | — | 0.05 | 0.3 |
| Least peel force | 0.5 | — | 2.0 | 0.05 | 0.3 |
| Shortest print | 0.3 | — | — | 1.5 | 0.3 |
| Auto, the goals' mean | 0.27 | 0.33 | 0.67 | 0.53 | 0.3 |

The model's length is the cube root of its bounding box, which does not turn with it, so
dividing by it changes no ranking — it only stops the weights meaning different things at
different sizes. Lower total wins. `orient-lab` (`docs/cli.md`) is what checks the weights.

A model on supports is lifted off the plate, so its underside hangs like any other
surface. Only a model printed straight on the plate rests on it: there a downward face
within 0.5 mm of the lowest point is footprint and not overhang, and a tip there is held.

## The candidates

The model's own flat faces first: normals are binned on a 16-per-axis lattice, the bins
are summed by area, and the largest 24 are kept. A model rests on its own faces, and a bin
is a hash lookup a face where clustering properly would be a comparison a cluster.

Then 200 directions from a Fibonacci lattice, which needs no triangulation and clumps at
no pole. That is what covers a model with no flat face at all.

Anything within 4 degrees of a direction already kept is dropped, and the flats are added
first, so a sphere sample that lands on a face loses to the face.

## Two passes

The peel term needs the model cut; the other three are read off the faces without turning
the mesh at all, since area and normals do not turn with it. So every candidate is scored
on the three cheap terms plus a **shadow** — half the area the faces project onto the
plate, which is the cross-section exactly for a convex model and an upper bound otherwise
— and only the best few are cut at 24 heights to measure the section for real: the area its
contours cover, by the rule the rasteriser fills by, so bodies overlapping at that height
are counted once (ADR 0206).

Without the shadow the first pass would rank on footprint and height alone, and would send
a slab into the second pass lying flat, which is the one orientation the peel cannot take.

The cost is a bias: a candidate whose shadow badly overstates its section can be cut from
the shortlist before it is ever measured. See `docs/decisions/0087`.

The same cut finds **suction cups**: a hole in a section whose inside point stands, in
the section under it, on material or on a hole already sealed. On the plate the first
section's holes are sealed by the plate; on supports air reaches them from below. It is
the rule `core-analysis` follows on the cured stack, read at 24 sections instead of
every layer, so a vent thinner than the spacing between two of them is missed.

## The build volume

Where the settings carry a build volume, every first-pass candidate is checked against it
(ADR 0288). The check reads an envelope: the model's farthest vertex along each of 256
Fibonacci directions, built once per model. That is a convex polytope inside the hull,
short of it by at most a hundredth of the model's radius, so every extent is read that
much larger.

- **Height:** the envelope's extent along the way down, plus the lift onto supports.
- **Plan:** the envelope's shadow. A pose on supports is given a raft's 2 mm margin on
  every side for the supports' feet.

The plan may fit any way the model is turned about the vertical. `fitting_turn` takes its
convex hull and the turns where a caliper of the turned box comes to lie along a hull edge.
Between two of those, each caliper rests on one vertex, so the turned width is a sinusoid
`d.x cos t - d.y sin t` and the depth is another. A fit, if there is one, starts at an end
of a span or where one of the sinusoids meets its wall, and those points are solved
exactly. A pose that fits no way fails the print (step 1 below). The shortlist takes the
poses that fit ahead of those that do not, and the rotation of a pose includes the turn
that fits it. The proof prints the best pose that fits even when none of the finalists
does, and checks what it printed, supports and all, the same exact way.

## The poses a person tries

Three kinds of candidate are cut whatever the first pass makes of them (ADR 0277):

- the model leaned 15°, 30° and 45° from the way it stands, eight ways round. A leaning
  model's shadow is far larger than any of its sections, so the first pass ranks the very
  pose that cuts the peel last;
- the two best on each reading alone: overhang, shadow, stack height;
- each drain hole pointed at the plate (below).

## Resting on the plate

A model drawn to print as it stands should not be lifted. Where the settings allow it,
the way the model stands and its six largest flat faces are scored set on the plate. A
pose is a seat when a face lies flat on it and either under a hundredth of the surface
hangs or a quarter of its shadow is flat on the plate; it stays there with supports only
under what hangs over it — what a person does with a figure on its base (ADR 0279).

Seats are ranked with the lifted poses by the same rule, each goal on its own reading: a
seat's predicted support stands from the plate, a lifted pose's from its lift, so the
fewest supports compares the two directly (ADR 0280). The proof always prints the best
seat, wherever it ranks, as the search predicts a seat's supports less surely. Whichever way the proof stands
a pose is the way the window and the command line stand it: on the plate, or lifted by the
support profile's own lift.

Set on the plate with no face flat on it, a model is balanced on an edge or a point: that
counts as a failure, under a cup and a tearing pull.

## A model as it prints

What the search is handed is the model the way it will cure, not the way its file drew it
(ADR 0276). The window bakes a scale or a stretch into the mesh first, so every
millimetre above is one on the plate.

A hollowed model keeps two readings apart. Supports stand under the outer surface, so the
first pass reads that alone. The second pass cuts the shell with its drain holes and
channels appended, wound inward: a ring pulls a fraction of the disc it is the rim of, and
the cavity is a cup until a hole lets the air in. Two things keep that reading honest:

- a shell is cut every millimetre, up to 128 sections, because a cavity's floor is one
  wall thick and a section that misses it leaves the cavity looking open from below;
- every run of cuts gets its own planes half a millimetre apart, up to 16, so that one of
  them meets the hole inside the wall it goes through.

A hole vents a cavity from its first layer only when it points almost straight at the
plate, far finer than the sphere is sampled. So each hole pointed down is a candidate of
its own, and is always among those cut.

## Choosing

Every stage ranks by one rule, `core_plate::rank` (ADR 0275, 0278, 0279): the search on what
it predicts, the proof on what it printed. In order:

1. what fails the print, compared exactly: a model set on the plate with no face flat on
   it; a pose past the build volume however it is turned, by its envelope in the search,
   and printed with its supports in the proof. For every goal but the
   fewest supports, suction cups and pulls past the 30 N `core-analysis` names fail it too;
   the fewest supports leaves a cup to a drain hole, the shortest print takes the pull;
2. islands and contacts left hanging, within a quarter and two of the fewest, where a
   stage reads them;
3. the goal's own reading, within a tenth of the best and the noise of its unit: a
   sliver of the dimensionless term, half a newton or 0.1 mm of stack in the search;
   0.05 ml of support, half a newton or 0.3 mm of stack once printed;
4. within that band, the fewest islands and hanging, then the tie-break: the total in
   the search; in the proof, the goal's reading to the noise of its unit and then the
   search's own order.

The model stays as it stands unless a turn fails less, standing is out of the band, or
the turn wins 5 % of the goal's own reading and more than the noise of its unit; the
turn is weighed against the best ranked way of standing, on the plate or on supports. No
goal weighs which way is up: the file does not say.

A goal may have **enough**: a reading at or under which every candidate reads alike, and a
**fallback** goal whose reading then decides, ahead of the tie-break (ADR 0281). The least
peel has enough at 10 N, a third of the 30 N warning, and falls back on the fewest
supports: under 10 N a model is turned only to save support, and a turn between two that
both have enough has to save 5 % of it and more than its noise. Every stage hands the rule
every goal's reading of a candidate and the noise of each in its own unit, so a policy
that weighs several goals is one more `Policy`, with no stage changed.

The shortest print reads the stack — the model and the lift under it, millimetres — which
the layers and the time follow, since a layer takes as long whatever its area. Its band
is a fiftieth of the shortest and a millimetre, some four minutes on a 100 mm figure, and
it weighs no support. A pull past the limit fails no pose for it: the limit is an estimate
good to a factor of three, and the goal is the fewest layers that hold (ADR 0283).

The fewest supports skips steps 2 and 3: past what fails the print, the least support
volume wins outright, and any turn that needs less is worth making. Islands, contacts with
no room and cups are its user's to settle with the tools that fix them.

### Balancing every goal

Auto (ADR 0284) asks for no goal in particular. It weighs each goal's reading as a share of
the best that any pose read, among the poses that fail least and leave least hanging, and
it takes the pose with the smallest product of shares:

```
cost = Σ ln((r_i + floor_i) / (best_i + floor_i))
```

A reading past enough counts as enough, so the least peel's reading is never under 10 N.
The floor is what no longer matters: ten struts of support, and five noises of stack,
which at 1.5 mm printed is about what the bottom layers add to any print. The product does
not depend on the unit a goal is read in. A goal on which every pose reads alike adds
nothing. A pose twice the support and half the stack ties with the one it trades against.

In order: what fails the print, with cups and tearing pulls fatal as for the least peel;
islands and contacts left hanging, as for every banded goal; poses inside every goal's own
band at once, with the fewest supports given a tenth and its noise; poses that no sound pose
beats on every goal past its noise; then the cost; then the largest single share; then the
tie-break. The model stays as it stands if that is inside every band. Otherwise it turns
to a pose inside every band, or for a twentieth less cost and more than the noise of every
goal together.

A goal's band is the edge of what reads alike, not a price. Dividing by bands lets the
narrowest one choose, and that was why the first design was dropped. Weights taken from how
widely each goal spreads, or from best and worst, move with whichever absurd pose the search
happened to cut.

Every finalist auto would offer as a trade, beaten on every goal by no sound pose, is
ruled `Alternative`; one that is beaten is `Dominated`. `Ruling::is_offered` covers those
and every goal's poses as good as its first. The window steps through them beside the
choice.

A goal is ranked by nothing another goal sets. What is its own is an exhaustive `match` on
`OrientGoal`, so a new goal is forced through every one: its weights (`OrientGoal::weights`),
its reading off the mesh (`Faces::reading`) and once printed (`Printout::reading`) with the
noise of its unit (`Printout::noise`), its policy with its band, enough and floor
(`Policy::of`), and its finalists' spacing (`apart_deg`). Auto weighs it with the rest from
there, with no change of its own. Both readings take the goal as an argument, so every
goal's reading of one candidate is at hand to a goal that weighs several. What every goal shares — steps 1 to 4,
the build volume, supports placed on the print's layer — moves every goal when it changes,
and is measured on all of them in `orient-lab`.

A wider shortlist and a pattern search around the winner were measured and dropped:
the better the search found low scores, the more of what it found the support generator
left hanging (ADR 0275).

## Proving

The score is a prediction, and what it predicts is measured cheaply once a printer is
known. `core-engine` takes the search's finalists — best first, no two within 10°, 20° for
the fewest supports, whose volume changes slowly with a turn; for auto, each goal's own best
right behind the first — and prints the first few
and the way the model stands, with supports stood under each, and every pose that may rest
on the plate set there, with supports only under what hangs over it. A few is four for a model of a million faces and up to sixteen for a
lighter one under 200 mm, which prints as many in the same time.

The supports are placed on the print's own layer (ADR 0279): how many contacts placement
puts down follows the layer it reads, and on a stack three times coarser a figure leaned
45° read 189 contacts where the print stands 442. Placed on the print's layer, a trial's
contacts, hanging contacts and support resin are the print's exactly. The cure that
follows is cut coarse: the tallest pose into about 160 layers, none thicker than 0.15 mm,
cured at a 0.2 mm pixel, and all of them at once. It reads the cups and tearing pulls a
print does.

Islands it cannot read: a print's islands are specks the support generator leaves alone,
which only the print's own pixel and layer resolve together (ADR 0285). A piece of a layer
is born only where the surface turns from going down to going up, so the proof opens a
window 1.5 mm round every low point of the model, and the middle of every flat underside
narrower than 1.5 mm, a few layers high. It cuts the window's faces on the print's planes, draws
the contours that close inside it on the print's pixel grid, and keeps each piece standing
on nothing. Each is checked again on the whole plate's contours at its layer and the one
under it, since a wall reaching in from outside can hold up what the window drew alone.

A pose set on the plate gets supports only under what hangs over it, the raft with them
where the profile has one, and none where nothing hangs. If a contact there finds no room
for a column, the pose fails every goal that weighs cups, as a cup would. All are ranked
by the rule above on what they printed — support resin, newtons, millimetres of stack —
and `prove` answers every finalist with what decided its place, `core_plate::rulings`.

The command line orients by the search alone, and stands a model the search set on the
plate the way the proof does, `stand_seated`: supports under what hangs, none if nothing does.

## Arranging

Footprints are bitmaps, not polygons: one bit a cell of `cell_mm` (1 mm), packed into
64-bit words a row, so testing a part against the plate is an `and` of a few words a row.
A triangle marks the cells its centre-test covers and the cells its edges cross, so a
sliver thinner than a cell is still covered.

A clearance is a dilation of the part before it is painted onto the plate, which is why
`Mask::grown` returns a mask bigger on every side rather than one of the same size.

Packing is first-fit by decreasing area, each part as near the near left corner as it
fits, and the packed block is moved to the middle of the plate once at the end. Packing
from the middle instead splits the free space in two and loses a plate of three models to
one. Why bitmaps rather than no-fit polygons: `docs/decisions/0088`.
