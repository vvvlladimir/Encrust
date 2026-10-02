# 0128. The exposure follows the layer in sight

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

ADR 0091 made the exposure follow the layer height: a resin measured at 0.05 mm for 2.5 s
is written at 5 s on a 0.1 mm layer. It did that at write time, inside `PrintJob`. In the
window, the Print settings panel kept showing the resin's 2.5 s while the file got 5 s,
and the only sign of it was a line of grey text under the layer height field. The resin
editor had the same gap: changing the height a resin was measured at left its exposure
alone, which silently changes what every layer cures to.

## Decision

Changing the layer height carries the normal exposure, and every exposure band, along
the resin's working curve right away, in the numbers the user sees. The session's resin
copy is then *measured at* the new height (`MaterialProfile::rescaled_to`), so the writer
scales by exactly one and the file is the same file it was before. The bottom exposure is
not carried: it is exposed to stick to the plate, as ADR 0091 already says.

A carried number says so until it is touched. Its label is in the warning colour, a line
says what it was and at which height, and a button puts that value back while keeping the
new height. Editing the number by hand, or picking another resin or printer, ends that.
Changing the height twice keeps the value from before the first change, and going back to
that height restores it exactly rather than carrying it there and back. The resin editor
does the same with the height a profile is measured at.

`exposure_for_mm` moves to `printer-profiles` beside `MaterialProfile`: how an exposure
follows a thickness is a property of the resin. `core-format` re-exports it.

## Consequences

`Slicing` loses its own `layer_height_mm` field: the height is the session resin's, so the
two can no longer disagree. A project still records its bands at the resin's measured
height (`bands_as_measured`), so project files keep their format and old ones load the same.
Loading a project cut off the resin's height shows the carried exposure straight away,
which is the point.

The CLI is unchanged: it has no panel, and its warning at write time stands.

## Alternatives considered

### Show the written exposure but leave the resin copy alone

A read-only "will be written at 5 s" beside an editable 2.5 s. Two numbers for one thing,
and the one you can edit is not the one that is printed. It also gives no answer to "keep
my 2.5 s at this height", which is what the back button is.

### Carry every setting of the resin

Lift, waits and light power do not depend on the thickness of the layer, and the bottom
exposure is deliberately not compensated. Carrying them would be motion without physics.

### The option that won, and what it costs

The session resin's `layer_height_mm` no longer means "where the resin was measured" but
"what this session cuts at", which is a second meaning for one field. The base resin keeps
the first, and `bands_as_measured` has to read it from there.
