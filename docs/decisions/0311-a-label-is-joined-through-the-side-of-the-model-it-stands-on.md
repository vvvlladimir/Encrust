# 0311. Join a label through the side of the model it stands on

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

A label is joined with `combine_exact` (ADR 0303, 0304), and every pass through the engine
costs the faces of the whole model however small the letters: reading a million-face model
in takes about 1.2 s, a boolean with it about as long again, so writing on a dense scan took
two and a half seconds after the union was made to take the model in once rather than three
times. The letters touch only the faces near them.

## Decision

`combine_exact_toward(models, op, toward)` cuts the first model with `core_geometry::cut`
by a plane square to `toward` 0.5 mm under the lowest point of the others, sends only the
side they stand on through the engine with them, and stitches the answer to the other side
along the cut: the upper cap is the faces of the answer lying in the cut, the lower one the
faces of the other side with every corner on that line facing out of it, and the two must
meet point for point and walk every edge of the line back to each other. Where the side cut
off holds more than half the model's vertices, the cut leaves a loop open or the stitch does
not close, the whole model is combined instead; where the engine refuses the side cut off,
the plane is tried three times more a few hundredths of a millimetre lower first. Union and
difference only. The text tool passes the normal it was clicked on.

## Consequences

On a million-face model a label is joined in 0.1 to 0.5 s instead of about 2.3 s, as fast as
the model is sparse near the letters, and answers the volume the whole cut does. The model
gains a line of split faces along the cut, coplanar and invisible. A cut that fails costs
up to four cuts and small booleans before the whole one, about a tenth more. The stitch
checks itself, so a wrong join falls back rather than lands; a cut whose cap the engine will
not read at any of the four heights is a defect of `cut` on vertices lying in its plane,
and is the signal to fix that rather than add heights.

## Alternatives considered

### A box round the letters

Would take less of a model the letters stand on edge-on, but its six cuts meet at its edges
and leave a face split on one side of a seam and whole on the other.

### Keeping the model read in between stamps

Saves the 1.2 s of reading in, not the boolean, and holds the engine's copy of the model,
some hundreds of megabytes, for as long as the tool is open.

### The option that won, and what it costs

A plane takes everything on the letters' side of it: a label in a hollow, or on a wall with
an arm in front of it, takes that too, and past half the model gains nothing.
