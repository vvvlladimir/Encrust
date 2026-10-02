# 0091. A stack is a plan of boundaries, not a count times a height

- **Status:** Accepted
- **Date:** 2026-09-25

## Context

Until now a stack was a layer height and a number of layers: `Windows` held sampling
planes, `PrintJob` held a count, and every Z in a written file was `(index + 1) * height`.
Adaptive layer height breaks all three at once — the plate stops at heights no single
number produces, each layer is priced at its own thickness, and each needs its own
exposure.

The adaptive rule itself is settled science: the cusp a stair step leaves is the layer
height times the cosine of the angle between the surface normal and Z (Dolenc & Mäkelä),
so a wall takes the thickest layer going and a shallow slope the thinnest. Its degenerate
case is the flat face, where that cosine is one and the formula asks for the thinnest
layer the machine has — when what a flat face actually wants is a boundary on itself.

Firmware is the other constraint. The research behind step 12 found machines that ignore a
layer's own Z and step the plate by the header's height instead, which prints an adaptive
stack at the wrong scale, silently.

## Decision

`core-slicer::LayerPlan` is the stack: the boundaries between layers, lowest first, plus
the height the mesh itself stops at. `bounds[i]` to `bounds[i + 1]` is layer `i`; the
sampling plane is the middle of that band, clipped to the ceiling so a partial top layer
is still sampled inside the material. `PrintJob` carries the plan and derives its layer
count, its Z per layer and its height from it; the header states the plan's thickest layer.

`core_slicer::adaptive_plan` walks the model bottom to top. Faces are scattered into
`min_height_mm`-tall slots of a 1D grid, each slot keeping the steepest lean crossing it,
and a layer takes the largest whole number of `min_height_mm` that every slot it spans
allows. A face within a thousandth of horizontal is left out of that grid and its height
recorded instead: a layer that can land on it does, exactly.

`PrinterProfile::firmware` carries what a machine obeys beyond its header —
`per_layer_settings` and `variable_layer_height` — and `core_format::validate` refuses a
stack of mixed thicknesses on a machine that does not claim the second. No shipped profile
claims it: the evidence is that firmware support is inconsistent even within one
manufacturer's range, so it is the user's own tested claim to make.

Exposure follows the thickness, measured against the height the resin profile was
calibrated at — `exposure_for_mm` scales linearly, or by the resin's working curve where it
states a penetration depth `Dp`. A file header states the plan's thickest layer and that
layer's exposure, so a uniform stack still agrees with its own header. The bottom block is
never compensated: it is exposed to stick to the plate, not to cure through.

## Consequences

A uniform stack is a plan whose layers are all the same, so one code path serves both, and
the boundaries are the same grid the old `(index + 1) * height` produced. The last layer of
a model that does not end on a boundary now travels in full rather than being clipped,
which is what the plate actually does.

Slicing off the resin's own height stops printing at the resin's own seconds. A resin
measured at 0.05 mm for 2.7 s, sliced at 0.1 mm, is now written at 5.4 s where it used to
be written at 2.7 s and under-cure. That is a change to what an existing `--layer-height`
run produces, so both binaries say it out loud rather than letting it happen quietly, and
the CLI no longer rewrites the resin's calibrated height to whatever it sliced at.

Because every thickness is a whole number of the floor, a ceiling that is not one is never
reached: 0.05 mm over a 0.023 mm floor tops out at 0.046 mm, and the exposure is scaled
against that. `AdaptiveSettings::reachable_max_mm` is what both binaries warn from.

`core-format` gains a dependency on `core-slicer` and re-exports `LayerPlan`, so the two
format crates get the type without a dependency of their own. The plan is a `Vec` of
boundaries held for the write, forty kilobytes on a ten-thousand-layer stack, against the
single float it replaces.

Supports, the drainage scan and trapped-resin detection still read a stack cut at one
thickness. They are analyses over the model, not the print, and their sampling density
does not have to match it — but it means a support plan is not pinned to the exact layers
that will be exposed. Revisit that when supports learn to read a plan.

Adaptive slicing is a quality setting, not a speed one. On a sphere at a 0.03 mm cusp it
produces more layers than a uniform 0.1 mm stack, not fewer: the poles are shallow and pay
for it. It saves layers only where the model is mostly vertical.

## Alternatives considered

### Keep the scalar height and carry a Z table beside it

Smaller diff: `PrintJob` keeps `layer_count` and gains a `Vec<f32>` of tops. Rejected on
two sources of truth — a count and a table that can disagree, with nothing but a validation
to catch it.

### Decide thickness after slicing, by comparing consecutive layers

Stack layers, XOR them, erode, and merge while the difference vanishes.
It needs no normals and works on any input. Rejected because it needs the whole uniform
stack cut first, which is the expensive half done twice, and the workspace has no reason to
hold a stack (ADR 0066).

### Compensate against the plan's thickest layer

It leaves a plain `--layer-height` run writing exactly the bytes it wrote before, because
nothing is measured against the resin. Rejected once the arithmetic was looked at: the
anchor then moves with the settings, so a ceiling of 0.05 mm over a floor of 0.023 mm
anchors 2.7 s to a 0.046 mm layer and over-exposes the whole stack by nine per cent. The
resin's own calibration is the only thickness its seconds are known to be right for.

### The decision above, and what it costs

Exposure now moves when the layer height does, which is right and is also a surprise to
anyone whose habit is to change `--layer-height` and expect the seconds to stay put. The
warning is the whole defence, and a warning is easy to scroll past.

The cusp rule reads the model's normals, so it is only as good as its triangles: a coarse
STL of a curved surface reports the facet's lean, not the surface's, and plans to the
facet. A flat face is found by its normal too, so a surface a hair off horizontal is not a
flat face and gets the thinnest layers instead of a boundary — the threshold is a
thousandth, and a model tilted a degree off axis loses the benefit entirely.
