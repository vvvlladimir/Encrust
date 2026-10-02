# 0130. Flare a pillar into its pad at forty-five degrees

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

A support meshed by `docs/decisions/0044` meets its foot on a square shoulder: the pillar
is a cylinder of `middle.diameter_mm` and the pad is a tube of `platform_diameter_mm`
underneath it, so the join is a right angle all the way round with nothing carrying the
load across it. Two things follow. It is the corner a support snaps at, because a cured
right angle concentrates the peel force on one layer. And it does not look like what every
other MSLA slicer draws, which is a cone from the pillar out to the pad.

The pad itself was a cylinder with a chamfer on the last `FOOT_BEVEL` of its height, so it
reads as two pieces rather than one: a vertical band with a step into the bevel, which is
visible in the viewport at the shipped 1 mm thickness.

The profile has no field for either. `docs/decisions/0044` said the signal to add one
would be someone reporting a foot that will not come off; the signal that arrived instead
is the shape of the join.

## Decision

**`[bottom]` gains `upper_diameter_mm` and `lower_diameter_mm`**, the widths of a cone
between the pillar and the pad. The cone **rises as far as it widens**, so its height is
`(lower - upper) / 2` and its wall is at forty-five degrees. That is not a field: the angle
is the decision, and a height field would let the two disagree.

The flare is a tube of its own, drawn with the pillar's `facets` rather than the pad's side
count, so a cube or a prism foot still carries a round flare. It sinks `FOOT_BITE` of the
pad's height into it, which is the overlap rule of `docs/decisions/0041`. It is clamped
three ways: never narrower than the trunk standing in it, never wider than the narrowest
ring of the pad under it, and never rising above the root it widens — a column barely
taller than its own foot keeps whatever rise is left, or none.

**A round pad is one trapezoid**: its rim at the top, `rim - FOOT_BEVEL × height` where it
meets the plate. The vertical band is gone. `FOOT_BEVEL` goes from 0.35 to 0.6 with it:
now that the whole wall is the bevel, the constant is the tangent of that wall's lean, and
0.6 stands it at 59 degrees off the plate — a wedge, where 0.35 was a pad with its corners
knocked off. A tapering foot keeps three rings, because narrowing towards the support and
bevelling at the plate cannot both be done with two.

The Supports window draws both, and its ten callouts are the ten a user carrying numbers
across will recognise, in the same left and right columns.

## Consequences

The join carries load over a cone instead of a corner, and the drawing in the window
follows the mesh, so a user setting the two diameters sees the angle they get.

Existing profiles load unchanged: both fields are `serde(default)`, at 1.0 and 2.2 mm,
which is the shipped medium pillar flared to the preset default. Validation puts them
in the chain it already enforces — `upper ≤ lower ≤ platform_diameter_mm` — and the window
restores that order the way it restores every other pair.

The pad meets the plate on 72% of the area it did, at the shipped 8 mm foot on a 1 mm
platform. That is grip traded for release, and the signal to trade it back is a plate that
drops a part rather than a foot that will not come off.

The flare costs one tube per support standing on the plate: `facets × 2` quads, 24 at the
shipped twelve, and the resin the cone holds. Every support on a plate pays it, so a plate
of a thousand supports is a thousand more tubes to sweep and to slice.

Reopen this if the forty-five degrees is the wrong angle for a resin — the signal is a
support that still snaps at the pad, or one whose flare will not release from it. The field
to add then is the angle, not the height.

## Alternatives considered

### A fillet rather than a cone

A curve tangent to both the pillar and the pad spreads the stress better than a cone does.
Rejected because it is rings of a torus fitted between two radii, and the sweep takes a
list of rings with no notion of tangency; it would be several rings where the cone is one,
on the one piece of geometry every support on the plate carries.

### The angle as a profile field, the height derived from it

One more number and every angle is reachable. Rejected because no slicer asks for it, the
answer is always forty-five, and a profile that sets it to eighty degrees builds a pad the
flare pokes out of the side of.

### Two diameters and a derived forty-five degrees, which is what we do

The honest cost: a user who wants a taller flare has to widen it, because the two are the
same number. The angle is a constant chosen to match other slicers rather than measured
against anything that snapped, exactly like `FOOT_BEVEL` and `FOOT_BITE` beside it.
