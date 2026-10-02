# Layer preview

How the window shows one layer of the sliced stack. The exposure mask lives in
`encrust-app/src/panels/inspector/mask.rs`, the scrubber in
`encrust-app/src/panels/section.rs`, the state behind both in `encrust-app/src/preview.rs`,
and the shrinking step in `core-raster/src/preview.rs`.

## The stack

There is no stack. `PreviewJob` merges everything visible on the plate and works out the
layer heights, which is all the slider and the rail need; the layer being shown is cut
when it is shown, as the window of 64 layers it falls in, and that one window is what is
kept between frames. Stepping inside it costs nothing; crossing into the next one cuts
again. See ADR 0068.

`stack_fingerprint` hashes what would change the layers — the layer height, and the
identity, mesh, placement and visibility of every visible object — so the panel can say
what it is showing is stale. Selecting an object and moving the camera are deliberately
not in the hash: they change nothing about the contours.

Export cuts the plate its own way, a window at a time straight into the file, so nothing
is handed over from the preview.

## From contours to a picture

Three steps, on the frame the layer, the scene or the panel changes:

1. `ScanlineRasterizer::rasterize` turns the layer's contours into `LayerRuns`, using the
   printer's `RasterSettings` — the same panel size, pitch and shading the `.goo` export
   uses, with the panel mirroring off (ADR 0109) and the rows flipped, so a part standing
   right and far on the plate is drawn right and high in the mask, as the viewport has it
   (ADR 0134).
2. `core_raster::downsample` shrinks the runs to a `LayerMask` of at most 2048 pixels a
   side, keeping the brightest pixel of each block (ADR 0023).
3. The mask becomes an 8-bit `egui::ColorImage` and a texture with nearest-neighbour
   filtering.

The pane zooms about the cursor and pans by dragging. Once the view holds no more of the
panel than a texture may, it is drawn from the full-resolution runs, cut by
`core_raster::crop` with room to pan in, rather than from the shrunk mask: the grey of an
edge and its blur live inside one pixel of that, and its brightest-pixel shrink hides them.

The runs and the texture are cached against `(fingerprint, layer index, raster settings)`, so holding
the slider still costs nothing and dragging it costs one rasterisation a layer.

## What the panel reports

The scrubber is the vertical rail over the right of the viewport, not a strip under it, and
it is drawn in both modes; see `docs/decisions/0061`. While the Preview mode is open the
layer it is parked on is also where the viewport cuts the models, so the mask in the
inspector and the model beside it are showing the same height of the same part.

The slider counts layers from 1, and the readout gives the sampling height `z` in
millimetres above the plate — the middle of the layer's band of material, as
`docs/design/slicing.md` describes — and the area the layer exposes in square millimetres.

That area comes from the window that has been cut, as half the sum of the layer's signed
double areas, so holes subtract themselves. It is blank for the frame before the picture
appears, because nothing has been cut yet. It is not read off the preview mask: the shrink keeps the brightest
pixel of each block, which inflates area on any panel large enough to be shrunk.

## Drawing it

The panel's pixels are not square in millimetres on every printer, so the picture is sized
from the area it covers — `width_px * pitch.x` by `height_px * pitch.y` — rather than from
the texture's own proportions.
