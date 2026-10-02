# 0144. Compensation belongs to the resin on a machine

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

A part does not come off the plate at the size it was sliced at. The established answer is
four groups of settings — shrinkage per axis, tolerance on holes and outer walls,
the same again for the bottom block, and a per-layer time that corrects the estimate — and
keeps them in the printer configuration.

Two of the four are properties of the resin: how much it pulls in as it cures, and how far
its cure bleeds past the mask. The other two depend on the machine as well: the light
engine sets the bleed, and the motion hardware sets the time a layer really costs. Neither
is a property of the model or of one job.

## Decision

`printer_profiles::Compensation` is one struct on `MaterialProfile`, overridable per
machine through `PrinterTuning` like every other number a machine changes about a resin.
There is no per-job switch: the same answer is right for every file that resin prints on
that machine.

Shrinkage is applied as a scale of the printed geometry, **per object**, taken about the
middle of that object's footprint and about the plate. A correction must resize a part
without sliding its neighbours or lifting it off the plate. `merge_plate_compensated`
applies it as it bakes the plate; the CLI applies it to its one mesh after supports are
built, so a support is corrected with the part it holds.

The tolerance offsets and the parity offset are stored and calculated
(`Compensation::offsets_of_layer_mm`) but not yet applied: the contour offsetting they need
is its own piece of work.

The unaccounted per-layer time is added by `PrintJob::print_time_s` and reaches the file's
header, because the header's time is the same estimate.

The two calibrations that can be measured get a calculator each, over the resin form:
drawn against measured per axis, and predicted against the clock over a known stack. Both
write into the fields rather than beside them.

## Consequences

A resin retuned on a second machine gets its own corrections there, and the first
machine's are untouched, for free. A user with one machine never sees the distinction.

The written file changes only where a correction is set. Every shipped profile is neutral,
so nothing anyone has sliced moves.

The tolerance fields are in the format before anything reads them. That is the cost of
landing the profile shape once rather than twice, and it is why they have no row in the
form yet: a knob that does nothing is worse than a missing one.

The signal to reopen: a correction that turns out to depend on the model rather than on the
resin and machine — a tolerance that has to differ between two parts on one plate — which
would mean this belongs beside the exposure bands instead.

## Alternatives considered

### Keep them on the printer profile

One place to look, and the machine is what the user blames. It lost because shrinkage is
the resin's, and a machine running three resins would need three sets of numbers under one
profile — which is `PrinterTuning` turned inside out.

### Scale the whole plate at once rather than each object

One transform, no per-object bookkeeping, and identical for the single centred part that
is the common case. It lost because a plate of several parts would have them drift
outwards from the plate centre by the correction, which is a millimetre at the edge of a
Saturn 4 Ultra and can push a part off the build area.

### The option that won, and what it costs

Shrinkage is applied where the geometry is baked, so it is done twice — once in the window
and once in the CLI — and the two have to agree. The shared part is the arithmetic
(`Compensation::placement`), which is in `printer-profiles` where neither front end can
drift from it, but the call itself is duplicated.
