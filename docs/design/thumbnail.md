# The sliced file's thumbnail

How the picture inside a `.goo` or `.ctb` file is made. The renderer is
`core-thumbnail/src/render.rs` and the files beside it in `render/`, the image and its
resampling `core-thumbnail/src/image.rs`,
and the one caller is `core-engine/src/run.rs`, which renders it for every front end. Why
it is a software renderer rather than the viewport's is in ADR 0048.

## What is drawn

A `Part` is a borrowed `Mesh` plus the `Transform` that places it on the plate, so a plate
is drawn without being merged into one mesh: `core_engine::parts` hands over every model
on the plate with its placement, and every support mesh with the identity transform, all
as `Arc` clones.

The plate itself, the grid and the build volume are not drawn. The picture is the part on
a flat background.

## The view

Orthographic, from `EYE = (1, -1, 0.75)` in plate coordinates with Z up: the front right
corner, a little above the part. The basis is built the usual way,

```
forward = -normalize(EYE)
right   = normalize(forward × Z)
up      = right × forward
```

and every vertex becomes `(dot(v, right), dot(v, up), dot(v, forward))`. The third
component is the depth, growing away from the eye, and the first two are what lands on
pixels. Because the basis is orthonormal, a normal in view coordinates is the plate normal
rotated, so shading works in view space.

The frame fits the part, not the build volume: the projected bounds of every vertex are
measured, and the scale is whichever of the two axes is tighter, with a 6% margin on each
side. A 20 mm model and a 200 mm model therefore fill the same thumbnail. A part seen
exactly edge on spans nothing along one axis; the other one still sets the scale, and a
part that spans nothing at all is drawn as a point rather than dividing by zero.

## Filling a triangle

Shading is deferred: the faces are filled first into a depth and a normal per pixel, and
every covered pixel is lit once afterwards (`render/raster.rs`, `render/shade.rs`).

A part placed anywhere but where its vertices stand is copied into plate coordinates, and
its normals are smoothed with `core_geometry::smooth_normals` across every edge whose faces
turn less than 40°: a ball's facets blend into one surface, a box keeps its edges. A corner
the smoothing skips, on a face of no area, takes the face's own normal.

Each face is filled over its screen bounding box with the standard edge function test. The
three edge values are divided by the signed area of the triangle, which normalises both
windings: a mesh whose faces were never oriented fills the same pixels as one that was.
Depth and normal are interpolated from the three barycentric weights, and a pixel is only
written when it is nearer than what is already there.

## Lighting

Per pixel, the normal is turned towards the eye first, so an unrepaired mesh shows its back
faces lit as fronts and still reads as a solid. Then, in view space:

| Term | Direction | Share |
|---|---|---|
| Ambient | — | 24% of the model colour |
| Key, diffuse | high over the camera's left shoulder | up to 80% |
| Fill, diffuse | low on the camera's right | up to 25% |
| Key, highlight | Blinn, exponent 40 | up to 22% towards white |
| Rim | `(1 - n·v)³`, strongest seen edge on | up to 45% towards white |

The key sits high so a box's top, left and right faces come out at about 96%, 57% and 49%:
three tones, not two. The rim light lifts the outline off a dark background, where a dark
line would vanish. A lit pixel is never black, which the window's recent-file pictures rely
on to tell a part from the background.

## Lines

Where one surface passes in front of another, a pixel keeps 40% of its colour. A jump is
the second difference of depth two pixels either side, across and down, deeper than six
pixel widths and than 2.5% of the parts' largest extent: a plane has none however steep it
stands, and the pores and cracks of a sculpted surface are shallower than the share, so only
a limb in front of a body draws a line. A neighbour with no surface draws none; the
silhouette is the rim light's. Supports take no line and no occlusion: on their thin struts
either turns into a dark mesh.

## Colours and shut-in surfaces

Models are drawn in `RESIN` and supports in `SUPPORT`, both in `core-thumbnail/src/settings.rs`:
the window's dark viewport takes its own colours from there, so a file's picture shows the
plate as it was set up. `core_engine::parts` marks each mesh with its `Look`.

A covered pixel's diffuse light is scaled by how shut in it is (`render/occlusion.rs`): for
four directions and two radii, 1.4% and 2.2% of the image width, wider than a sculpt's pores, the mean depth of the two
opposite samples is compared with the pixel's. Standing nearer the eye by 0.6 radii shuts it
in fully along that pair, past 3 radii it is another part in front and counts as open. The
mean over the pairs takes up to 33% of the light. A plane or a ridge stays open; the floor of
a groove and the folds of a figure darken.

## Cost

A 980k-triangle sphere renders, is cut to a 400 x 300 record and dithered in 148 ms at 1024
pixels square, single-threaded but for the normal smoothing, on an Apple M1: 51 ms flat-shaded at
1024, 32 ms at 512. It runs once per exported file, beside seconds of slicing, which is why
the rest is not parallel.

## From one image to every record

One 1024-pixel square image is rendered per job, and with it a coverage mask: the pixels
whose depth a face wrote. Each format then cuts its own records out of it with
`Thumbnail::fitted_to` (ADR 0402):

| Format | Records |
|---|---|
| `.goo` | 116 x 116 and 290 x 290, RGB565, big-endian, uncompressed at fixed offsets |
| `.ctb` | 400 x 300 and 200 x 125, run-length RGB15, at offsets the header carries |

For a render, `fitted_to` crops to the box around the covered pixels, scales that box to fill
the record less a 6% margin on each side, keeping its aspect ratio, and centres it. An
uncovered pixel counts as the record's background, so the render's own background never
reaches a file and a 4:3 record shows a wide part across its width rather than a square with
bars beside it. A picture read from a file has no mask: it is fitted whole and padded.

A writer that packs pixels into 16-bit words, `.goo`, `.ctb`, `.cxdlp`, `.photons` and the
Anycubic tables, first calls `Thumbnail::dithered`: a 4 x 4 ordered dither adds up to one
step of the kept bits, so a gradient packs to a fine pattern instead of bands, and black,
which gains less than a step, stays black and still packs into runs.

The resampling is a box filter — every source pixel inside the destination pixel's
footprint, averaged. A point sample at this reduction drops a thin support out of the
picture altogether.

The render frames the parts to 88% of its longer side, about 900 pixels, more than the
longest side of every record (800), so a record is always a reduction of what was rendered
and never a magnification of it.

`PrintJob::thumbnail` is optional. `None` writes a blank record of the right size at the
offset the header points at, which is what the writers' own tests use and what any caller
with no geometry to draw falls back to.
