# 0031. Measure an overhang over a rise, not from one layer to the next

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

`docs/decisions/0029` settled that placement reads the slice stack rather than the mesh's
faces: what needs holding up is area that appeared with nothing cured under it. The first
implementation asked two questions of each layer — is this piece an island, and does this
piece reach more than a fixed 1.5 mm past the layer below — and the second question was
wrong in both directions.

A fixed reach is not an angle. At 0.05 mm layers a surface leaning 45 degrees moves
0.05 mm sideways per layer and a surface leaning 88 degrees moves 1.43 mm: neither reaches
1.5 mm, so a surface that is nearly a ceiling passed the test. Lower the threshold to
catch it and every wall fails, because the measurement is below the resolution it is being
made at. One cell of the grid layers are read on is 0.1 mm, which is two layers of lean at
45 degrees and one at 63 degrees. There is no threshold that separates those.

A profile also has to be able to say what angle it will tolerate. `max_overhang_deg` is the
number every other slicer exposes and the number a user reaches for; a reach in millimetres
per layer is not something anyone can set.

## Decision

An overhang is material that has moved further sideways than the profile's angle allows,
measured over a fixed rise of one millimetre — `REFERENCE_RISE_MM` — rather than over one
layer.

Each layer is compared against the layer a millimetre below it, the *reference*. The
allowed reach is `REFERENCE_RISE_MM * tan(max_overhang_deg)`, so 45 degrees allows a
millimetre and 60 degrees allows 1.73 mm, and at 0.1 mm cells those are ten and seventeen
cells: differences the grid resolves cleanly. Near the bottom of a stack, where there is
less than a millimetre of model underneath, the reach is scaled down to the rise actually
available.

Both questions are then asked of the same material: the cells this layer is the first to
cover, `layer` less the layer directly below it. That is the only new underside a layer
has. A piece of it that touches nothing the layer below carries is an **island**; the rest
is an **overhang** wherever it lies further than the reach from the reference layer.

A contact is put a whole layer below the plane the layer was cut at, not half a layer. A
cell this layer is the first to cover has the model's own surface somewhere inside that
rise, and a contact placed above it lands a column on the surface it is meant to hold up.

## Consequences

- The profile's angle means what it says. A 30 degree profile places more supports than a
  60 degree one, and both are testable against a ball, whose lower cap passes through every
  angle.
- A wall standing on itself adds no new underside, so the whole of a layer that has not
  moved costs one pass over its spans and nothing else. That is most layers of most models.
- What is measured is where the *material* is, not where a face points. A staircase of
  one-cell steps and a smooth slope of the same angle are the same thing to this test,
  which is the point: what tears is unsupported area, not steep triangles.
- The size filter had to change with it. The old one asked whether the overhang's area was
  worth a column, which made sense when the region was a millimetre thick; the region is
  now one layer of growth thick, so the filter asks how far across the piece is instead —
  `min_overhang_mm`, half of what one support head carries.
- The reference rise is a constant, not a setting. If a printer arrives whose layers are
  thick enough that a millimetre is only a few of them, the constant is the thing to
  revisit.
- Every contact now sits up to one layer height below the surface it holds. The tip cone
  sinks `contact_depth_mm` — 0.15 mm and up — into the model, which is three times that at
  0.05 mm layers, so the tip still meets the part.

## Alternatives considered

### Keep a reach per layer, and scale it by the layer height

Arithmetically the same idea, and it was what the code did when the constant was 1.5 mm.
It fails on resolution: at any sane layer height the per-layer reach of an angle worth
allowing is under one cell of the grid, so the comparison is between two numbers that both
round to zero. Measuring over a rise that is ten cells tall is what makes the comparison
mean anything.

### Read the angle off the mesh's faces after all

Cheap, and it is what an FDM slicer does. It answers the wrong question for MSLA —
`docs/decisions/0029` has the argument — and it cannot see an island at all, which is the
failure that actually loses parts.

### Measure against the layer below, and accumulate

Track how far each piece has crept and support it once the total passes a threshold. That
is the same measurement with state attached, and the state is what makes it unrepeatable:
a run started at a different layer, or stopped and restarted, would answer differently.

### The option that won, and what it costs

A millimetre of rise is a window, and a window lags. A surface that turns from vertical to
horizontal inside one millimetre is caught a fraction of a millimetre late, because for
part of that rise the reference layer is still the vertical part. The thickness of one
layer of growth is also the thinnest an overhang region can be, so a surface leaning just
past the limit produces a ring one cell wide on every other layer, and it is the
`min_overhang_mm` filter rather than the area that decides whether that ring is worth a
column. Both are accepted: the lag is under the head's own diameter, and the filter is
measured in the same unit the head is.
