# 0318. Press a relief only where it is painted, by moving the painted faces

- **Status:** Accepted
- **Date:** 2026-10-20

## Context

A relief moved every face its texture reached, held back only by the angle mask of ADR
0308. A texture wanted on one panel of a model had to be pressed everywhere or not at all,
and the press cut a field over the whole model and meshed all of it again: a scan of a
million faces lost its detail everywhere and took as long for a thumbnail-sized patch as
for the whole surface. The support brush already paints a set of faces (ADR 0092), but
`Region` lived in `core-supports`, a peer of `core-volume`, and the window had begun to use
it for the Measure and Position tools as well. Holding the unpainted faces in the field, as
the mask holds level ones, was tried first: their planes cut the relief wherever the
surface curves the wrong way, which on a scan of tiny faces scored the patch with streaks.

## Decision

`Region`, with its `brush`, `flood` and `submesh`, moves down into `core-geometry`; its
brush takes the mesh, hierarchy and transform instead of `core-supports`'s `Placed`, and
`Blocked` stays in `core-supports`. `core-volume` gains `press_painted` and
`press_painted_tiled`, which build no field: they split each painted face the mask does not
hold until no edge of it is longer than the lattice the painted area would be cut on,
splitting its unpainted neighbour along the shared edge, and move every corner inside the
paint along its normal by the height read there. Corners on the edge of the paint stay. The
Relief tool's Texture mode gains a brush, alt erasing, whose faces each model keeps as
`relief_painted`, cleared when the mesh is replaced and written to the project manifest
under that key, absent while empty. The view draws the texture only over painted faces. The
texture is picked from a thumbnail that opens the gallery, and the brush is a
`toggle_button`. While the brush is in hand, nothing painted means nowhere; with it put
down, nothing painted presses through the field, as before. See `docs/design/volume.md`.

## Consequences

Everything outside the paint is the model's own mesh, and a patch costs its own area: the
`field` benchmark presses an 8 mm patch of a 40 mm ball in 8 ms against 1.7 s for the whole
ball through the field. The relief tapers over one split face at the edge of the paint
instead of stopping square. Faces are only halved, never flipped or remeshed even, so the
slivers of a CAD fan, an inside corner or a crease can fold the surface through itself,
which the field never does; this is accepted for now. A print failing there reopens this,
for a remesh into even triangles before the press or a return to the field. A painted press
keeps no basis for the next. A project from an earlier build reads as nothing painted, so
the manifest stays at version 6.

## Alternatives considered

### Holding the unpainted faces in the field

No new algorithm, a square edge, and no folds; but it remeshes the whole model at the
lattice for any patch, and the plane cut that keeps a held face flat scores a curved one.

### A field round the patch, joined to the model by an exact boolean

A square edge with no folds, but the boolean still walks the whole model, and the pressed
surface meets the model's own along the edge of the paint, where it leaves slivers.

### The option that won, and what it costs

A second way to press, with its own splitting, that can fold a surface where the field
could not, and a soft edge where the field gives a square one.
