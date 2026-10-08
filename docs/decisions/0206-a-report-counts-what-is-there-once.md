# 0206. A report counts what is there once, and resin only from the masks

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

Three figures in the reports were sums over things that overlap, and a manual run caught all
three on one coupon:

- `sliced volume`, the CLI's resin line, added the signed areas of each layer's contours.
  Where two bodies overlap it counted the shared area twice, and since ADR 0188 lays a cut
  under every body it meets, a drain hole was subtracted eight times. On a 30 mm ball,
  hollowed with a hive and drilled, it read 6120 mm³ against the 6245 mm³ the masks cure —
  two resin figures in one report, 2 % apart, neither labelled as the other's rival.
- `peak_section_mm2`, the peel term the orientation search minimises, added the same signed
  areas. A bracket of three overlapping boxes measured 151.5 mm² where its profile is
  148.5 mm².
- `inspect`'s `volume` is `signed_volume`, the divergence theorem over every face. A coupon
  of 26 overlapping shells read 3301.9 mm³ where the print cures 3.0 ml.

The masks are already the authority on resin: ADR 0163 has `core-analysis` fold the volume
the file header, the weight and the price are taken from, pixel by pixel, where a body laid
over another lights the pixel once and a cut laid eight times darkens it once.

## Decision

**Resin is counted from the masks and nowhere else.** `Sliced::resin_volume_mm3` and the
CLI's `sliced volume` line are gone, and the batch JSON's `slicing.resin_mm3` with them; the
figure a report states is `cured volume`, `cured.resin_mm3`, and the summary of a batch adds
those up. A run with no printer has no panel to draw on and so states no resin at all.

**`core_slicer::covered_area(&[Contour]) -> Scalar` is the area one plane covers**, by the
positive winding rule `core-raster` fills by, over `i_overlay`, which `core-slicer` already
carries for `offset_contours`. The orientation search measures each section with it.

**A mesh volume stays a sum over shells, and says when it is one.** `inspect` prints
`volume 3301.900 mm^3 (26 shells added up, so what they share counts twice)` where two
shells that both enclose material have boxes that meet. A cavity is wound inwards and
encloses none, so a hollow model says nothing.

**An island taken out of the file is a part, not a layer of one.** `Measured` keeps what the
layer below lost and counts a removed piece as a new island only where it touches none of
it, so a part floating over 60 layers reports `islands 1 taken out of the file, over 60
layers` rather than 60 islands.

**Layers that cure nothing are a counter, not a `slice defect`.** A gap under a floating part
is a legitimate stack, so `empty layers` is a line of its own among the counters.

## Consequences

Every number a report states about an area or a volume now counts material once, and the two
resin figures that disagreed are one figure. A farm gating on `slicing.resin_mm3` has to read
`cured.resin_mm3`, which was beside it and already right.

`estimate` and `slice` without a printer lose their last volume figure. That is honest — a
stack is contours until a panel turns it into pixels — but it does mean the one way to price
a model is to name the machine it is for.

The orientation search pays a polygon overlay per section, 24 sections for each of 8
shortlisted candidates, on top of the cuts it already makes. The run is dominated by the
cuts and the measurement is now the shape's own area rather than an upper bound of it.

A box is the whole overlap test for `inspect`: two shells whose boxes meet but whose material
does not will carry the note anyway. The note says what the figure is, not that the model is
wrong, so a false one costs a reader nothing but a second's thought.

The signal to reopen this: a user wanting resin out of a run with no machine. That would mean
rasterising to a panel of our own choosing rather than counting contours again.

## Alternatives considered

### Keeping the contour volume and uniting the contours per layer

`covered_area` over every layer of every stack would have made the two figures agree exactly.
It also pays a polygon overlay a layer for a number the rasteriser works out as a side effect
of the work it has to do anyway, which is the definition of counting twice.

### Renaming the contour figure rather than removing it

`contour volume` beside `cured volume` would have kept a number some users had got used to.
Two volumes in one report invite the question of which is right, and the answer would always
have been the other one.

### A true union volume for `inspect`

The honest fix for a mesh of overlapping shells is a boolean union, or a voxel count through
`core-volume`. A boolean is a dependency the workspace does not have in three dimensions, and
a voxel count would make `inspect` — the cheap command, the one a farm runs over a directory
— build a distance field. Saying what the figure is costs a clause.

### The option that won, and what it costs

A report has fewer numbers in it than it did, and one of them moved house: the resin a run
states now depends on there being a printer, which reads as a regression to anyone who had
`--json` piped into a spreadsheet. What they were reading was wrong by a per cent or two in
the direction nobody notices, which is exactly how it survived this long.
