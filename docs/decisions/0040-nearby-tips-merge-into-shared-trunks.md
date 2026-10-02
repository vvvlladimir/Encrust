# 0040. Merge nearby tips into shared trunks, greedily, highest meeting point first

- **Status:** Accepted
- **Date:** 2026-09-19

## Context

Step 7b fills a model with vertical columns, one per tip. On a part with a large overhang
that is a bed of parallel pillars: they cost resin, they cost peel force on every layer,
and they take a long time to cut off. Every production slicer merges them. The question is
by which rule.

Two families of algorithm are in use.

The first is layer-based, from Thomas Rahm's work on tree supports. It grows an
*influence area* per branch down the stack of layers,
intersecting it with an *avoidance area* that keeps it clear of the model, and merges two
branches when their influence areas overlap. It is a polygon algorithm over the whole
stack, and its output is a set of per-layer regions.

The second is the geometric one from *Clever Support: Efficient Support Structure
Generation for Digital Fabrication* (Vanek, Galicia and Benes, Eurographics Symposium on
Geometry Processing 2014). Each tip owns a cone of directions it can descend along,
bounded by a maximum lean from vertical. Two tips merge where their cones first meet, a
trunk grows from there, and the trunk is itself a tip for the next round. The paper
reports 40.4% less support material and 29.4% less print time than straight pillars.

The constraint this project already carries is `docs/decisions/0026`: a support is a mesh,
merged into the model before slicing. Nothing downstream knows supports exist.

The workspace also has no polygon boolean dependency, by `docs/decisions/0032`.

## Decision

Tips merge by the **Clever Support** rule, greedily, **highest meeting point first**.

`core_supports::grow` takes the `Column`s step 7b resolved and answers with a forest of
`SupportTree`s. Every node of a tree hangs from the node below it, so the parent link
points down and the root is at the bottom.

Each tip starts as a *front* at its contact. For a pair of fronts `p` and `q`, the meeting
point is solved in the vertical plane through them: with `t` the ground a strut covers per
millimetre it descends, the meeting point lies `(d + (z_p - z_q) · t) / 2` along the ground
between them, clamped to that ground, and as low as either descent can reach it. `t` comes
from `branching.max_angle_deg`.

A pair is offered only when the horizontal distance between them is within
`branching.max_merge_distance_mm`, and only when neither strut would have zero length.
A candidate is accepted only when

- one ray along each strut's own axis clears the model by `branching.clearance_mm`, and
- the meeting point has somewhere to stand, by the same `landing` every column uses.

The new trunk's radius is `(r_a^k + r_b^k)^(1/k)` with `k = branching.trunk_exponent`,
capped at `branching.max_trunk_diameter_mm` and never below either branch.

Candidates sit in a max-heap keyed on the height of the meeting point, and fronts sit in a
grid of the merge distance so a new one asks nine buckets rather than every front placed
so far. A candidate whose endpoints have since merged is dropped when it is popped.

A surviving front drops a vertical trunk to its landing, which is where a support still
lands on the model rather than on the plate, exactly as in step 7a.

`branching.enabled = false` skips the whole merge, and every tip comes back as a one-node
tree: the vertical column of step 7a, unchanged.

## Consequences

A support is no longer a column, so `Column` stops being the thing that is meshed and
picked. It stays as the record of where a tip is and where a plain vertical drop from it
would land, which is what `grow` is seeded with.

Struts lean. That is the degree of freedom the vertical column of `docs/decisions/0028`
never had, and `branching.max_angle_deg` is the one knob that governs it. A lone tip with
nothing to merge with is still vertical, so `0028` is narrowed rather than superseded: the
contact is still anchored in model space, and the trunk under a support is still vertical.

On a 40 mm ball standing 10 mm clear of the plate, automatic placement puts down 37 tips.
They merge onto 5 trunks in 98 µs and the support mesh drops from 2138 mm3 to 920 mm3,
57% less, which is in the range the paper reports. `cargo bench -p core-supports --bench
branch` prints that line.

The clearance test is one ray along each strut's axis. A strut running alongside a surface
without crossing it passes, and so does a strut that grazes a feature thinner than the
epsilon the ray starts past. That is the known weak point, the same shape of weakness the
coverage test in `docs/decisions/0033` already has. The signal to reopen it is a branch
visibly cutting a corner of a part in the viewport.

The greedy choice is not optimal. Merging the highest pair first can leave a third tip
stranded next to a trunk it could have joined lower down, and the heap never revisits an
accepted merge. Reopen with a benchmark if the trunk count stops falling on real models.

## Alternatives considered

### Layer-based influence areas

The best-tested implementation there is, and it handles avoidance of the model exactly
rather than by sampling. Rejected on two counts. It is a polygon boolean algorithm and
this workspace deliberately has none, by `docs/decisions/0032`; adding one back for
supports alone would be a large dependency for one feature. And its output is per-layer
regions, which would have to be meshed anyway to satisfy `docs/decisions/0026`, so the
polygon work would be thrown away at the end.

### A minimum spanning tree over the tips, cut by the angle limit

Cheap, one pass, and it gives the globally shortest set of connections. Rejected because
an MST connects tips, not the *points where their descents can meet*: an edge between two
tips at the same height is horizontal, which is not a strut that prints. Repairing that
turns it back into the cone solve above, without the property that made the MST attractive.

### Clever Support, greedily, highest first, which is what we do

The honest costs are the three above: greedy is not optimal, the clearance test is a
single ray per strut rather than a swept volume, and the whole merge is sequential. The
sequential part matters: step 7b's placement runs on every core, and `grow` does not,
because the heap has to be drained in order for a run to be repeatable. At 98 µs against
the tens of milliseconds placement itself takes, that has not been worth fixing.
