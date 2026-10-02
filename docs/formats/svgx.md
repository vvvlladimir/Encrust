# The `.svgx`

What the FlashForge Foto, Focus, Explorer and Hunter and the Voxelab Proxima read: a
twenty-eight byte header, two bitmap previews, and an SVG document holding the whole stack.
Nine machines in the shipped catalogue carry `output = "svgx"`.

It is the one container we write whose layers are **vectors**. A layer is a group of filled
polygons in millimetres, not a grid of pixels, so it carries no grey at all: a path is
filled or it is not.

## Where the knowledge comes from

| Source | What it settles |
|---|---|
| An open-source reader and writer of the container | The header, the preview record, the element and attribute names of the document |
| The bitmap specification | That a preview record is a plain 24-bit bitmap, bottom row first |

Three things worth stating before the tables:

1. **Coordinates are millimetres from the middle of the panel.** `x_mm = x_px × pitch −
   width_mm / 2`, which is why the document has to state both the panel's pixels and its
   millimetres; a file short of either cannot be turned into masks by anybody.
2. **A path is filled even-odd.** A ring inside another is therefore a hole whatever order
   the two are written in, which is what lets a writer put every ring of a layer into one
   path without working out which sits inside which (ADR 0169).
3. **The document is a stream.** It is the whole of a stack and can run to tens of
   megabytes, so a reader of ours indexes it in one pass — the settings and one offset per
   layer — and parses a group only when its layer is asked for.

## The header

Little-endian.

| Offset | Type | Field |
|---|---|---|
| 0x00 | 16 bytes | `DLP-II 1.1\n`, padded with nuls |
| 0x10 | u32 | address of the first preview |
| 0x14 | u32 | address of the second preview |
| 0x18 | u32 | address of the document |

## A preview

A 24-bit bitmap at 128×128 and then 200×240: the standard fifty-four byte header, then
three bytes a pixel, blue first, with the bottom row of the image first. Both widths are a
multiple of four pixels, so no row needs the padding a bitmap would otherwise take.

## The document

```xml
<?xml version="1.0" encoding="utf-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg">
<printparams machinename="Foto 8.9" materialname="Standard grey" layerheight="0.050"
  volume="000000012.500" layercount="240" lightintensity="1" resolutionx="3840"
  resolutiony="2400" displaywidth="192.000" displayheight="120.000" machinez="200.00">
<projectiontime attachlayer="5" buildinlayer="0" attachtime="32.00" basetime="2.60" />
<projectionadjust x="100" y="100" />
<printrange minx="-96.000" miny="-60.000" minz="0" maxx="96.000" maxy="60.000" maxz="12.000" />
</printparams>
<g id="background">
</g>
<g id="layer-0" area="412.500" perimeter="98.000">
<path d="M -1.2 -0.6 L 1.2 -0.6 1.2 0.6 -1.2 0.6 Z " style="fill:white" fill-rule="evenodd" />
</g>
</svg>
```

`attachlayer` and `attachtime` are the bottom block's count and exposure, `basetime` the
normal exposure, `buildinlayer` the transition count. A group's `area` and `perimeter` are
what the layer cures in square millimetres and the length of its outline in millimetres; a
vendor slicer writes other units there, and nothing in the firmware is known to read either
beyond showing them.

**`volume` is a fixed-width zero-padded field** of thirteen characters. The resin a stack
takes is only known once the last layer has been measured, and the field stands in front of
every layer group, so it is reserved at that width and written again in place at the end —
which keeps the document a stream rather than something held in memory (ADR 0169).

`printrange` is the panel, not the model: the extents of a model are not known to a writer
that is handed one layer at a time, and the field is informational.

## Writing a layer

Our pipeline hands a writer an exposure mask, so the polygons are traced out of it. Every
boundary between a lit pixel and a dark one is a unit step on the pixel grid; the steps are
oriented so that material lies to one side, stitched into closed rings, and a step that only
continues the one before it is dropped — a rectangle comes out as four corners. A ring that
encloses material runs one way round and a hole the other, which is what the reader takes
the winding from.

A mask shaded by coverage is cut at a grey of 128: half a pixel's worth of coverage is
material and less is not, because the container has nowhere to put the difference.

## Reading a layer

A group's paths are split into subpaths at every `M` and `Z`, each becomes a contour in
plate millimetres, and the set is filled by `core-raster`'s scanline rasteriser at the
panel the document states, binary rather than coverage-shaded. Only moves and lines are
read: a curve has no meaning on a pixel grid and no writer of this container emits one.
