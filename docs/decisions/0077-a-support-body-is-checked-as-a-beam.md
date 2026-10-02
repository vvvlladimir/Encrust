# 0077. Check a support body as a beam, not as its axis

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

Step 7c checked a branch strut with one ray along its axis, and checked no vertical
column at all: a column's drop ray only chose where it landed. A support body is a tube
of real radius, so both tests miss what the body runs into — a strut passing a wall
within its own radius, a trunk descending a millimetre from a bulge, a meeting point that
falls inside the model, where a forward ray reports the far wall and calls it clear air.
All three cure the support onto the part, which is the failure step 9 exists to end.

This replaces the clearance test of ADR 0040, whose own Consequences named the single
axis ray as one of its honest costs; the merge strategy that ADR decided stands.

The established answer is eight rays on a ring of the body's radius plus a safety
distance, with a separate check of the vertical pillar, and no polygon or field machinery
in that path either.

## Decision

A support body is tested as a `Beam` — the cone it sweeps, with the profile's
`clearance_mm` added to both radii — by nine rays: the axis and the eight on the ring. A first hit on a back face means the beam set off inside the
solid and fails with the rest.

Every body goes through it: each strut of a merge, the trunk under every landing, and the
foot on the plate, which is wider than anything above it and so reaches into parts a
column passed in clear air. `landing` takes the `Body` that will be printed and answers
for all of them.

Where a body is meant to meet the model it is exempt: the head at a tip is trimmed off
the beam by `top.length_mm`, and a landing stops the beam where the beam's own width
already reaches the face — `(radius + clearance) * tan(slope)` above it. A face leaning
more than `MAX_LANDING_SLOPE_DEG` from horizontal is no landing at all: a foot on it has
nothing underneath, and the body runs down the face instead of onto it.

`clearance_mm` moves from `[branching]` to the profile, because it is now what every
support keeps, branching or not. What happens to a tip that fails all of this is
ADR 0078.

## Consequences

A support that cannot be built clear of the model is not built where it was asked for;
ADR 0078 is what then happens to the tip. A part resting on the plate loses the supports
under its own skirt outright, because a pad cannot go where the part already is — which
is what the Lift button exists for.

Merges that the axis ray let through are now refused, so a forest goes flat and costs
more resin: `cargo bench -p core-supports --bench branch` on the lifted 40 mm ball went
from 6 trunks and 56% saved to 16 trunks and 33% — those struts were the ones running
along the ball's underside. ADR 0078 wins most of it back, at 13 trunks and 41%.
`clearance_mm` is the knob that trades the rest.

Nine rays per body against one costs a `Bvh` query each, and `grow` pays it several times
over for every merge it tries.

The beam is the regular body's width even where `[small_pillar]` will print something
thinner, because how thin that is depends on the tree's tip count, which is not known
when it lands. A small pillar is therefore refused in a gap it would have fitted. If that
shows up on real models, the landing has to be resolved after the forest is cut.

## Alternatives considered

### Per-layer avoidance masks, the way layer-based tree supports work

The model's slice stack, grown by the body's radius, answers the same question in
constant time per query and is already a `Field` in this crate. It loses manual
placement, which has no stack — a click on the model must answer before anything is
sliced — and it ties `grow` to `Sliced`, which is a dependency the crate does not need.
Worth reopening if a bench shows the rays dominating a placement run.

### A capsule or swept-cone intersection against the `Bvh`

Exact where a ring of rays is a sample. It needs a distance query per face and a new
traversal in `core-geometry`, against a `Bvh` built for rays, for an answer that only has
to be right to within a fraction of a body's width.

### The option that won, and what it costs

Eight rays sample a surface; they do not cover it. A spike thinner than the gap between
two rays, standing between the ring and the axis, is still missed, and the miss grows
with the body's radius. The clearance is what absorbs it: at 0.5 mm of air around a
0.8 mm trunk the widest unsampled gap is under half a millimetre of arc.
