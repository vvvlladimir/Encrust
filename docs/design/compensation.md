# Compensation

What comes off the plate is not what was sliced. The resin shrinks as it cures, the light
bleeds past the mask, the bottom block spreads under its long exposure, and the machine
spends time on a layer that no setting accounts for. `printer_profiles::Compensation`
carries the corrections, one set per resin, overridable per printer through
`PrinterTuning` — the same mechanism the exposure uses, because the answer depends on both
the bottle and the light engine.

Every field is neutral by default. A resin that states nothing slices exactly as it did.

## Shrinkage

`shrink_x_pct`, `shrink_y_pct`, `shrink_z_pct`: the geometry is scaled by
`percent / 100` before it is cut. Above 100 slices the part larger, so that it measures
right once the resin has pulled in.

Calibrate by printing a part of known size, curing it, and measuring it cold:

```
percent = 100 × drawn / measured
```

A 20 mm bar measuring 19.90 mm wants 100.503 %. The Compensation card works this out from
the two numbers per axis.

**Z is normally left at 100.** A layer's shrinkage along Z is taken up by the layer cured
on top of it, so the error that survives lands in XY. A Z figure is worth setting only for
a machine whose Z travel itself is off, which is a different fault with the same symptom.

The scale is taken **per object**, about the middle of that object's footprint and about
the plate: a part shrinks towards itself and is held at the plate while it prints, so a
correction must not slide its neighbours around or lift it off the plate.
`merge_plate_compensated` does this as it bakes each object in; the CLI does the same to
the one mesh it has, after supports are built, so the supports are corrected with the part
they hold.

## Tolerance

`hole_offset_mm` (`a`) and `outer_offset_mm` (`b`) move each layer's walls: `a` inwards on
a hole, `b` outwards on an outer contour, so a positive value on either makes the body
larger. `bottom_hole_offset_mm` and `bottom_outer_offset_mm` are the same for the bottom
block, where a negative `b` is what takes an elephant foot off.

`parity_offset_mm` is added to both on every second layer. An MSLA panel cannot draw an
edge finer than a pixel — 0.019 mm on a Saturn 4 Ultra — so an offset below that only
exists as an average over alternating layers.

`Compensation::offsets_of_layer_mm` gives a layer its pair, and `core-pipeline`'s
`Tolerance` applies them in `fold_group`, before the layer is rasterised and therefore
before it is measured: what the preview says the print cures is what the file carries.

The offsetting itself is `core_slicer::offset_contours`, over `i_overlay` (ADR 0145). A
whole plane goes in at once, because which contour is a hole in which is what decides
where a wall ends up. Two consequences follow from the geometry, not from a choice:

- A hole narrower than twice its offset **closes**, and a wall thinner than twice its
  outer offset **disappears**. Both are what the printer would have done anyway.
- Corners are cut off square rather than carried to a point: the join is a bevel, which
  is what every offsetting slicer defaults to and what avoids a spike at a sharp corner.

An offset costs one boolean pass per layer and runs inside the window the rasteriser is
already parallel over, so it follows the contours rather than the panel.

## Unaccounted time

`layer_time_s`: seconds the machine spends on a layer beyond the exposure, the waits and
the travel the settings describe. `PrintJob::print_time_s` adds it to every layer.

Calibrate from one print, taking both numbers from the same job:

```
seconds = (actual − predicted) / layers
```

A job predicted at 35 m 49 s that took 60 m 50 s over 200 layers is 7.51 s a layer. It
moves the estimate only — no exposure, no motion, nothing the part is made of.
