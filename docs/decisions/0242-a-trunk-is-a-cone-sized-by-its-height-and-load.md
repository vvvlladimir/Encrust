# 0242. Make a trunk a cone sized by its height and load, and no brace wider than it

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

ADR 0040 gave every merge node the radius `(r_a^k + r_b^k)^(1/k)`, capped at
`max_trunk_diameter_mm`, and kept that width all the way down. A trunk was a cylinder as
wide at the joint where its branches meet as at the plate, and every strut between two
merges was already thickened by the tips above it. The cap read as "the trunk's diameter"
rather than as a limit. A single column, however tall, stayed a pillar's width down to its
flare, and a tall thin column is the one that buckles on the peel.

## Decision

Only a trunk standing on the plate widens. Every joint keeps the width of the branches it
joins, so such a trunk leaves its top at the pillar's width and runs as a cone to its
base. A column with no merges is a trunk from its neck down.

A trunk standing on the model keeps its width all the way down and ends as the tip at the
top does, upside down: the cone to its contact, then a bite to a point, or flat on the
surface for a plane tip. The model carries it, so there is no peel to stiffen it against,
and a wide end only scars the part.

The base radius is `SupportProfile::trunk_base_radius_mm(height, tips)`:
`min(cap, max(pillar, height / 15 / 2, carried))`, where `carried` folds
`merged_radius_mm` over the tips the trunk holds and `15` is `TRUNK_SLENDERNESS`, the most
times its base diameter a trunk may stand. The flare and foot take the trunk's base, not
the root's width.

`landing` now takes the number of tips. On the plate its clearance test sweeps the cone
from the joint's width to the base the height asks for, and that base sets the pad's room;
on the model it sweeps the joint's width. A candidate merge is asked about with the tips of
both fronts.

A brace is never wider than either trunk where it meets it: its radius is the least of
`bracing.diameter_mm / 2` and the two trunks' meshed radii at its two ends, each trunk at
its own group's profile. A brace that would have swallowed a thin trunk — a small pillar,
or a pillar under a thick brace — is thinned to it rather than dropped.

## Consequences

Tall supports stand wider at the plate and narrower where they leave the model, so the
resin goes where the peel loads the column. `trunk_exponent` and the cap keep their fields
and meaning for the load; no profile changes. A saved tree keeps its node radii, which a
tree grown before this records as merged widths; it still meshes, at those widths.

## Alternatives considered

### Dropping a brace wider than a trunk, or forbidding it in the profile

Dropping it leaves the thinnest trunks, the ones that sway most, untied. A profile rule
that the brace be no wider than the pillar cannot see a small pillar or a trunk's cone.

### A fixed cone angle

A base growing by a set angle per millimetre is simpler, but says nothing about whether
the column stands; the slenderness ratio is what buckling depends on.

### Slenderness as a profile field

A user could tune it, at the cost of a field in every profile and the panel. One constant
until somebody needs a second value.

### Making a cone of every strut

Widening every branch by the tips above it is ADR 0040's model, which is what made the
cap read as a width and thickened struts the model sees most of.

### The option that won, and what it costs

The ratio of 15 is a judgement between resin and stiffness, not a measurement; a column
standing on a 0.3 mm pillar still has to grow to 2 mm by 30 mm tall, which costs resin a
short print never needed.
