# 0133. Round every height a file states to the micron

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

`LayerPlan` holds its boundaries as `f32` plate millimetres, counted up from the bottom of
the model. A part lifted 3.32 mm off the plate makes every boundary a number around ten,
and the difference of two of those is no longer the layer height that produced them:
0.05 mm comes back as 0.05000019.

That number reaches the header. A reader refuses to open the file at all — "the layer
height (0.05000019mm) have more decimal digits than the supported (3) digits" — and a
printer reading the per-layer table gets plate positions with the same dust on them. The
machines step in microns; the formats state heights as `f32` millimetres and every other
slicer writes three decimals.

## Decision

`PrintJob` rounds to the micron on the way out: `height_mm`, `layer_z_mm`,
`layer_height_mm` and `nominal_height_mm` each return `(mm * 1000).round() / 1000`. Both
writers already go through those four, so neither format repeats the rule.

The plan itself is untouched. Slicing, analysis and the preview keep the exact boundaries;
only what is written is rounded, and each height is rounded on its own, so a position is
never the sum of rounded thicknesses.

## Consequences

A stack standing anywhere on the plate states 0.050 mm and a position of 3.370 mm, and a
reader opens it. A layer height finer than a micron cannot be written — 0.0125 mm would
be stated as 0.013 — which is what the three-decimal formats allow anyway.

An adaptive plan's thicknesses are rounded individually, so the stated thicknesses need
not add up to the stated total height; the positions do, because each is rounded from the
true boundary rather than accumulated.

Reopen this if a format arrives that carries heights as integer microns or as `f64`, where
the rounding belongs in the codec instead.

## Alternatives considered

### Round the plan when it is built

One rounding, and the preview would agree with the file to the last digit. Rejected
because the plan drives slicing: moving a boundary by half a micron moves the plane the
layer is sampled at, for a problem that only exists in the file.

### Round in each writer

`.goo` and `.ctb` each state the height their own way, so the rule would be stated twice
and the next format would forget it.

### The option that won, and what it costs

`PrintJob`'s accessors now mean "what the file says" rather than "what the plan holds",
and a caller wanting the exact boundary has to go to the plan. The app's layer readout is
one such caller, and it wants the rounded number anyway.
