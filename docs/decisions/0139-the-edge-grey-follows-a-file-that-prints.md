# 0139. Let the edge carry every grey

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

ADR 0113 floored the mask at grey 128 on the reasoning that a dimmer pixel leaves resin
uncured rather than curing less, and admitted in its own last section that 128 was "a guess
dressed as a measurement". The signal it named for reopening was evidence from a real
machine.

That evidence arrived. The same model was sliced for an Elegoo Saturn 4 Ultra by the vendor
slicer and by Encrust, and the two `.goo` files decoded and compared layer by layer. The geometry
agrees: intersection over union between the lit masks is 0.966 to 0.997, every bounding box
within one pixel, no mirroring difference. What differs is the edge.

| | Vendor file | Encrust |
|---|---|---|
| distinct greys in a layer | 254, from 1 to 254 | 123, from 128 to 250 |
| grey pixels per boundary pixel | 1.15 | 0.48 |
| grey as a share of the lit area | 3.14 % | 1.32 % |

That file prints this machine well with greys all the way down to 1, so the floor's premise
does not hold here. Worse, a floor in the middle of the ramp is a cliff: coverage 0.51 is
written 130 and coverage 0.49 is written 0, a 130-level step across half a pixel, which
bands more visibly than no anti-aliasing at all.

The known-good file also records a blur level of 2 and a grey level of 0 in its header,
against our 0 and 8. The grey level is ours to correct. The blur level is not our radius:
the reference file's grey ramps are a median of 1 pixel wide, and a `blur_px` of 1 already
takes ours to 3.34 grey pixels per boundary pixel and 2 to 5.57. Whatever that file calls
level 2, it is not a box of radius 2, and matching the number would over-soften every edge
by three to five times.

## Decision

Two defaults change so that a file leaving Encrust matches what the machine is known to
print:

- `Display::grey_floor` defaults to **0**, keeping every grey. It stays a profile field, so
  a panel measured to need a floor asks for one, and `--grey-floor` still overrides it.
- The `.goo` header's grey-level field carries **0** when no ladder was applied, instead of
  the constant 8, which claimed a rounding that did not happen.

The edge fade stays off by default. Exact coverage with no floor measures 1.12 grey pixels
per boundary pixel against the reference file's 1.15, so the floor was the whole of the
difference and there is nothing left for a blur to close.

ADR 0113's `Grey` type, its ladder and the floor mechanism are unchanged; only the value the
floor starts at.

## Consequences

Every edge now carries the full ramp, so a mask has roughly twice the grey pixels and a
written file grows by about a third: run-length data is the bulk of a sliced file and grey
pixels are what break runs.

The risk ADR 0113 guarded against is now the user's to guard against: a panel that genuinely
does not cure a dim pixel writes `grey_floor` in its profile. The signal to revisit is a
measured machine whose thin edges come out liquid at a floor of zero, and the answer is that
profile's floor, not a new global default.

## Alternatives considered

### Keep the floor and raise it into the profiles that need it

The smallest change, and it keeps ADR 0113's shape. It lost because the default is what
almost every user gets, and the default was wrong for the one machine there is evidence
about.

### Floor at a value low enough to be harmless, 16 or 32

Keeps a cliff, just a shorter one, and still needs a number nobody measured. A cliff of 32
levels is a cliff.

### Match the reference file's blur level of 2 as well

The obvious reading of the header diff, and it was tried: `--blur 2` triples to quintuples
the grey at every edge, because our radius and the file's level are different units. It lost
on the measurement above.

### The option that won, and what it costs

A default of zero is as much a guess as 128 was, just a guess with one working file behind
it rather than none. A slow resin on a weak panel can now be asked for a grey it will not
cure, and the failure is the one ADR 0113 described: a wall that looks finished with liquid
behind it.
