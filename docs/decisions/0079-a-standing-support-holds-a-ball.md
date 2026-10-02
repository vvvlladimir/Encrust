# 0079. A standing support holds a ball, not a column of air

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

Automatic placement thins what it puts down by asking whether a support is already
standing near a sample. That test was a cylinder: `coverage_radius_mm` across, and
`coverage_rise_mm` — a whole spacing, about 5 mm — tall. A support placed under a low
ledge therefore counted as holding everything within 5 mm above it, which its tip comes
nowhere near.

A stepped model is where that shows. Three tiers 4 mm apart got 33, 11 and 8 supports;
the same tiers 6 mm apart, clear of the rise, got 33, 21 and 22. The upper ledges were
not less in need of holding, they were inside the cylinder.

Measured as a sphere, the influence radius of a point already printed below is
`sqrt(R² − Δz²)`, so what has climbed away from a support is on its own.

## Decision

The reach of a standing support falls off with the rise above it:
`sqrt(coverage_radius² − rise²)`, nothing at all past the radius. `coverage_rise_mm` is
gone; one radius is the whole rule, in three dimensions.

A seed sits on the surface while a placed contact sits a layer under it, so the rise is
still measured with that layer's allowance.

## Consequences

A ledge further above a support than the coverage radius is held on its own account,
which is what a stepped model needs. A slope still thins to one support per place: the
points it would repeat are within a fraction of a millimetre in every direction, well
inside the ball.

More supports go down on anything terraced, so a faceted model costs more resin than it
did — the same trade the density knob makes.

What this does not fix: a column passing unrelated material still suppresses samples on
it, because coverage is measured on the plate rather than on the part. The answer to that
is to drop standing points that fall outside the layer part being sampled.
That is still open, and it is what the rest of step 9b is for.

## Alternatives considered

### Keep the cylinder, shorten the rise

One constant, no new shape. Every value of it is arbitrary: too short and a slope gets a
support per layer, too tall and the next ledge up goes bare. The ball has no such knob —
the rise it tolerates is the radius it already has.

### Bind coverage to the connected part of the layer

Strictly better: it also fixes the unrelated-material case.
It needs the part a sample belongs to carried through placement, which is the larger of
the two changes; the ball is what makes the stepped model right on its own.

### The option that won, and what it costs

A ball is isotropic, and holding is not: a support reaches further sideways under a flat
ceiling than it does upwards. Using one radius for both means the sideways thinning is as
tight as the vertical one, so a wide flat overhang gets slightly more supports than the
carry figure alone would ask for.
