# The sliced file's thumbnail

How the picture inside a `.goo` or `.ctb` file is made. The renderer is
`core-thumbnail/src/render.rs`, the image and its resampling `core-thumbnail/src/image.rs`,
and the two callers are `encrust-cli/src/main.rs` and `encrust-app/src/job/pipeline.rs`. Why
it is a software renderer rather than the viewport's is in ADR 0048.

## What is drawn

A `Part` is a borrowed `Mesh` plus the `Transform` that places it on the plate, so a plate
is drawn without being merged into one mesh: the CLI hands over the single mesh it sliced,
and the window hands over every visible model with its placement and every support mesh
with the identity transform, all as `Arc` clones.

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
pixels. Because the basis is orthonormal, a face normal in view coordinates is the world
normal rotated, so shading works in view space and no vertex is transformed twice.

The frame fits the part, not the build volume: the projected bounds of every vertex are
measured, and the scale is whichever of the two axes is tighter, with a 6% margin on each
side. A 20 mm model and a 200 mm model therefore fill the same thumbnail. A part seen
exactly edge on spans nothing along one axis; the other one still sets the scale, and a
part that spans nothing at all is drawn as a point rather than dividing by zero.

## Filling a triangle

A depth buffer, one `f32` per pixel, starts at infinity. Each face is projected, shaded
once as a flat colour, and filled over its screen bounding box with the standard edge
function test. The three edge values are divided by the signed area of the triangle, which
normalises both windings: a mesh whose faces were never oriented fills the same pixels as
one that was. Depth is interpolated from the three barycentric weights, and a pixel is only
written when it is nearer than what is already there.

Shading is flat Lambert against a fixed light in view space, over the camera's left
shoulder, with 28% ambient. The dot product's sign is dropped: a repaired mesh points its
faces outwards, but a thumbnail of an unrepaired one should still read as a solid rather
than as a hole.

A 980k-triangle sphere renders in 39 ms at 512 pixels square, single-threaded. It runs
once per exported file, beside seconds of slicing, which is why there is no parallelism
here.

## From one image to four records

One 512-pixel square image is rendered per job. Each format then cuts its own records out
of it with `Thumbnail::fitted_to`:

| Format | Records |
|---|---|
| `.goo` | 116 x 116 and 290 x 290, RGB565, big-endian, uncompressed at fixed offsets |
| `.ctb` | 400 x 300 and 200 x 125, run-length RGB15, at offsets the header carries |

`fitted_to` keeps the aspect ratio, centres what fits and pads the rest with the record's
background, so a square image in a 4:3 record gains bars either side instead of a model
stretched sideways. The resampling is a box filter — every source pixel inside the
destination pixel's footprint, averaged. A point sample at this reduction drops a thin
support out of the picture altogether.

512 is larger than every record either format holds, so a record is always a reduction of
what was rendered and never a magnification of it.

`PrintJob::thumbnail` is optional. `None` writes a blank record of the right size at the
offset the header points at, which is what the writers' own tests use and what any caller
with no geometry to draw falls back to.
