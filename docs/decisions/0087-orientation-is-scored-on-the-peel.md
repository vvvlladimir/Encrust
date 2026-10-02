# 0087. Score an orientation on the peel, and measure it in two passes

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

`Tweaker-3`, which is what an FDM tool orients with, ranks an orientation on overhang
area, bottom area and contour length, with weights trained by an evolutionary algorithm.
None of those three is what an MSLA print fails on. Every layer here is pulled off the
film, and that force follows the cross-section being pulled: a 40 by 40 plate lying flat
tears off its supports where the same plate standing on its edge prints without trouble.
Printer vendors say the same thing in their own words — the Z cross-section is
the thing to keep small.

The cross-section is also the one term that cannot be read off the faces. Overhang area,
footprint and height are sums over normals, areas and projections, none of which turn when
the model turns, so a candidate costs one pass over the faces. A cross-section costs a
slice: the mesh has to be rotated and cut. At 200 candidates on a 400k-triangle model that
is not a thing to do per candidate.

Orientation is wanted from the window and from the CLI, and it reads both the mesh and the
slicer, which no existing crate is the home for.

## Decision

A new crate, `core-plate`, over `core-geometry` and `core-slicer`. It owns where a model
stands on the plate; arranging will join it.

The score has four dimensionless terms — overhang, peel, height, footprint — weighted
1.0, 0.6, 0.2 and −0.3, with the arithmetic in `docs/design/orientation.md`. A downward
face lying on the plate is footprint, not overhang.

The search runs in two passes. Every candidate is scored on the three cheap terms plus a
**shadow**: half the area the faces project onto the plate, which is the cross-section
exactly for a convex model and an upper bound for anything else. The best eight are then
rotated and cut at 24 heights, and the largest section measured there replaces the shadow.

Candidates are the model's own flat faces, found by binning normals and ranking the bins
by area, followed by 200 Fibonacci directions, with anything within 4 degrees of a
direction already kept dropped.

## Consequences

A slab is stood on its edge and a 6 by 6 by 80 column is printed upright, which is what a
resin user does by hand and the opposite of what an FDM tool would answer. The weights are
what makes that call, and they are set by eye on the fixtures, not trained: a user who
disagrees has no way to say so yet, since `OrientSettings` exposes the overhang angle and
the search budget but not the weights.

A ball of 102k faces orients in 139 ms (`cargo bench -p core-plate`), which is the
benchmark to check before anyone makes the search cleverer.

The shortlist is a bias, not just a saving: a candidate whose shadow badly overstates its
section — a hollow shell seen end-on, a model that is mostly holes — can be cut before it
is ever measured. The signal to reopen is a model that orients visibly wrong while its
second-best candidate is visibly right.

## Alternatives considered

### Score every candidate by slicing it

Exact, no shortlist bias. Rejected on cost: 200 rotations and 200 slice stacks of a
detailed model is minutes, against 139 ms, and orientation is a button a user presses
while looking at the plate.

### Candidates from the convex hull's normals, as `Tweaker-3` does

The usual answer, and it captures a resting pose for an organic model. Rejected because it
needs a hull and the model's own face bins already catch every flat a model would rest on;
the Fibonacci lattice covers the rest more evenly than a hull of a noisy scan would.

### Keep it in `core-supports`

No new crate, and every dependency already there. Rejected because the support crate would
then own something that is not about supports, and arranging would have to follow it in.
The honest cost of the crate that won is one more entry in the workspace and in the
dependency graph, for two functions today.

### The decision above, and what it costs

Four weights, tuned by eye, decide how a model prints, and nothing in the tests pins the
weights themselves — the tests pin the answers they currently give for a slab, a column
and a cube. Retuning a weight can therefore turn a passing suite into a differently
passing suite, and only the fixtures would notice.
