# 0143. Exposure follows the working curve, measured or assumed

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

ADR 0091 made `exposure_for_mm` scale an exposure "linearly, or by the resin's working
curve where it states `penetration_depth_mm`". No shipped resin states one, so every job
anyone has sliced took the linear branch.

Linear is the wrong law. Jacobs gives the cure depth as `Cd = Dp ln(E / Ec)`, so the dose a
layer needs is `E(L) = Ec exp(L / Dp)` — exponential in the thickness, not proportional to
it. The two disagree in the direction that matters. A resin measured at 5.2 s for 0.05 mm,
cut at 0.025 mm, is written at 2.6 s by the linear rule and wants about 4 s by the curve:
a layer that does not stick. Cut at 0.1 mm it is written at 10.4 s against about 8.6 s,
which is time spent bloating the part.

Practice says the same. AmeraLabs' calibration guide: below 0.1 mm, halving the layer
height wants about a quarter less exposure, not half. The common rule that doubling the
layer height wants about 20 % more time is the other end of the same curve. Neither is
anywhere near proportional.

The curve needs a `Dp`. Measuring it means a working-curve print per resin, which is a
calibration nobody has done yet and which this project does not ship a jig for.

## Decision

`exposure_for_mm` always follows the Jacobs curve. A resin that states no
`penetration_depth_mm` is carried on `ASSUMED_PENETRATION_DEPTH_MM`, 0.10 mm — the middle
of the 0.05 to 0.15 mm a pigmented 405 nm resin shows. The linear branch is deleted.

`penetration_depth_mm` stays an `Option` and keeps its meaning: measured, or not measured
yet. Nothing in the shipped profiles is filled in with a guess.

The bottom block is still not compensated, as ADR 0091 and 0128 already say: it is exposed
to stick to the plate, and the bottom-to-normal ratio observed in practice falls as the
layer height rises, which is what a constant bottom exposure already does.

## Consequences

Every file written from a resin without a measured `Dp` changes its exposure, unless it is
cut at exactly the height the resin was measured at, where the curve is one by construction
and the file is byte-identical. Uniform jobs at the resin's own height — the common case —
are untouched.

Adaptive stacks get the change for free: `PrintJob::for_thickness` is the same function.
So does the resin editor and the carried-exposure line in the Layers panel.

A wrong assumed `Dp` is now a wrong exposure rather than a missing feature, and the error
grows with the distance from the measured height. It is bounded in the safe direction for
thin layers, which is where an under-exposed layer loses the print.

The signal to reopen: a measured working curve for any resin we ship whose `Dp` is far
outside 0.05 to 0.15 mm, which would mean one assumed number cannot serve them all.

## Alternatives considered

### Keep linear as the fallback

Costs nothing and changes no existing file. It lost because it is not an approximation of
the curve, it is a different curve: it passes through zero at zero thickness, so a thin
layer is written at an exposure that cannot cure anything, and the finer the layer the
worse the error. Adaptive layer height makes exactly that case common.

### Fill `penetration_depth_mm` into every shipped resin

Would put the number where a user can see and edit it. It lost for now because writing
0.10 into a profile claims it was measured for that resin, which it was not. A constant
named for what it is says the truth; the field stays empty until someone prints the curve.

### The option that won, and what it costs

One assumed number stands behind every exposure the program scales, and it is invisible:
there is no field showing it and no warning that a job far off the measured height rests on
an assumption. A resin whose real `Dp` is 0.05 mm will be under-exposed at thicker layers
by as much as the linear rule was over-exposing them.
