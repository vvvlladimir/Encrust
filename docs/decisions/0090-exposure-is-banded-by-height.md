# 0090. Exposure varies by bands of height, and the bottom block is out of reach

- **Status:** Accepted
- **Date:** 2026-09-25

## Context

Step 12 asks for exposure that changes over the height of a print. Both writers already
put an exposure on every layer, so the question is only where the number a layer takes
comes from, and what is allowed to change it.

The slicers that vary exposure at all call this Height mode, and offer Model and
Cross-sectional Area modes beside it. Model mode does not vary exposure at all — their
documentation says per-object exposure is unsupported and offers a grey level instead,
because one flash exposes every part standing at that Z at once. Cross-sectional area is
a property of the sliced stack, not of the model, so it cannot be known before slicing.

A band starting at zero would cover the bottom layers, whose long exposure is what holds
the print on the plate.

## Decision

`core-format` owns `ExposureRange` and `ExposurePlan`, and `PrintJob` carries a plan. A
range is `[from_mm, to_mm)` above the plate and replaces the resin's exposure for every
layer whose top Z falls in it. Ranges may overlap and the last one given wins, so a narrow
correction lays over a wide band without cutting the wide one in two.

The bottom block and its transition layers keep the resin's own ramp whatever a band says.

`PrinterProfile::per_layer_settings` says whether a machine's firmware obeys the per-layer
tables; it defaults to true, and `core_format::validate` refuses a banded job on a machine
marked false rather than writing a file the machine would print at header exposure.
`.goo` sets advance mode whenever a job's exposure varies at all.

## Consequences

A band is an override on top of the resin, so a resin recalibrated or retuned for another
machine still carries the bands the user drew, and a job with no bands writes exactly the
bytes it wrote before.

The viewport washes each band over the height it covers in a tint of its own, and the
same tint marks the band's fields in the inspector, so the height a number applies to is
read off the model rather than off two millimetre figures. The wash stops where the bottom
block does, which is the one place the panel and the file would otherwise disagree.

The plan is job state rather than resin state: it is not saved with a profile, and it will
have to be part of the project file of step 14. The ranges are a `Vec` a handful long, not
a value per layer, so nothing here grows with the stack.

Motion — lift, retract, light-off delay — is not banded. Slowing the lift where the
cross-section is large is the analysis step 17 will have the numbers for, and banding it
by height now would be guessing at the same thing with a worse input.

If per-object exposure is ever wanted, it does not come through here: it is a grey level
on the mask, which is the rasteriser's business and belongs with the grey-level work of
step 16.

## Alternatives considered

### Bands stored in the resin profile

No plumbing: the writers already hold a `MaterialProfile`. Rejected because a band belongs
to a model, not to a resin — saving the resin would save one model's ramp into every
future print with that resin.

### A per-layer exposure table computed at slice time

Exactly what the file needs, and no lookup while writing. Rejected because it is a value
per layer held for the whole write, where the input is a few ranges, and because it would
have to be rebuilt whenever the layer height changed.

### The decision above, and what it costs

A lookup runs per layer while the file is written, walking the ranges backwards. It is a
handful of comparisons against a `Vec` that is never long, but it is work inside the write
loop that a precomputed table would not do.

A band is keyed on the layer's top Z, so changing the layer height moves which layers a
band covers. That is the right behaviour — the band is a height, not a layer index — but
it means a ramp tuned at 0.05 mm quietly covers different layers at 0.03 mm.
