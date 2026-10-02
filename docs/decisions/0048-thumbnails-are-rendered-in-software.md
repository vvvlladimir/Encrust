# 0048. Render the sliced file's thumbnail in software, in a crate of its own

- **Status:** Accepted
- **Date:** 2026-09-19

## Context

Both sliced-file formats carry preview images, and both have been writing them black since
the writers were first built: `.goo` has two fixed square records inside its header, 116 and
290 pixels a side, and `.ctb` has two run-length records, 400 by 300 and 200 by 125, that the
header addresses by offset. Neither is optional — the bytes are part of the container
whatever they hold — and a file whose previews are black shows up as a black rectangle in
any reader of the file and on the printer's own screen, which is the one place a user checks
that the file on the stick is the file they meant to print.

The picture has to come from somewhere, and the pipeline has two front ends. `encrust-cli`
writes sliced files with no window open, on a machine that may have no GPU at all;
`encrust-app` already renders the plate through wgpu, but the first rule of the architecture
is that no `core-*` crate may know about a graphics API. Whatever renders the thumbnail has
to be reachable from both, and the `PrintJob` that carries it down to the writers lives in
`core-format`, below both binaries.

The two formats disagree about the shape of a preview: one is square, the other is 4:3, and
they differ again in size. Rendering once per record would mean four renders per file and a
camera fitted four times.

## Decision

Thumbnails are rendered on the CPU by a new leaf-facing crate, `core-thumbnail`, which
depends on `core-geometry` and nothing else in the workspace.

`render(&[Part], &ThumbnailSettings) -> Thumbnail` projects every part through an
orthographic view from the front right corner of the plate, a little above it, fills each
triangle through a depth buffer and shades it flat and two-sided against a fixed light. The
view frames what it is given rather than the build volume: a part drawn to scale inside a
200 mm plate is a speck at thumbnail size. A `Part` is a borrowed mesh and its transform, so
nothing is copied and the plate never has to be merged for a picture.

One image is rendered, 512 pixels square, and each preview record is cut from it with
`Thumbnail::fitted_to`, a box filter that keeps the aspect ratio and pads what is left over
with the record's background. 512 is larger than every record either format holds, so a
record is always a reduction and never a magnification.

`PrintJob::thumbnail` is an `Option<Thumbnail>`. `None` keeps the old behaviour — a blank
record of the right size at the offset the header points at — so a writer's tests and any
caller that has no geometry to draw still produce a valid file.

## Consequences

The CLI and the window produce the same picture from the same code, and a headless machine
writes the same file as a desktop. No core crate gained a graphics dependency, and
`core-format` gained one workspace edge, to `core-thumbnail`, which it re-exports so that
`format-goo` and `format-ctb` name the type without depending on the renderer.

The cost is one more rasteriser to maintain, beside the layer one in `core-raster`. It is
about 200 lines and it is not on any hot path: a 980k-triangle sphere renders in 39 ms,
once per exported file, against seconds of slicing. It is single-threaded for that reason.

The picture is not what the viewport shows. The camera is fixed, so a part the user turned
to look at from behind is still drawn from the front right, and the shading is flat with no
plate, no grid and no supports colouring of its own. If users ask for the thumbnail to match
the view they set up, that is a second source for the image, not a change to this one: the
app would render its own and hand it to `PrintJob`, which already takes any `Thumbnail`.

`core-thumbnail` holds the image type as well as the renderer, so a second source would
reuse `Thumbnail` and `fitted_to` rather than introduce another image type.

## Alternatives considered

### The app renders the plate offscreen through wgpu

It would match the viewport exactly, including the material and the lighting the user is
already looking at, and it would reuse the renderer that exists. It lost on where it leaves
the CLI: a headless slice would have to keep writing black previews or grow a second,
different renderer anyway, and the picture in a file would then depend on which front end
wrote it. It also drags a GPU device and a readback into a code path that must work on a
build server.

### A silhouette of the slice stack seen from above

The writers already see every layer, so the stack could be folded into an image and poured
into the records after the last layer, with the seek machinery of ADR 0045 and no renderer
at all. It is the cheapest option by a wide margin. It lost because the result is a flat
grey blob: a top-down silhouette of a bust and of a cylinder of the same footprint are the
same picture, which defeats the purpose of a thumbnail — telling one file from another.

### The option that won, and what it costs

A software renderer is code we own and have to keep correct, and it will always look worse
than the GPU one: flat shading, no anti-aliasing, hard edges on a curved surface, and a
fixed camera that cannot be moved. It is also a second place where triangles are rasterised,
which is the kind of duplication rule 5 usually pushes back on — the two rasterisers share
nothing because one fills pixel coverage in millimetres for exposure and the other fills
shaded depth in a picture. Anti-aliasing the thumbnail, if it is ever wanted, is a
supersample inside this crate and not a change anywhere else.
