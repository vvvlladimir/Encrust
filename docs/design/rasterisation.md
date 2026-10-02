# Rasterising a layer into an exposure mask

How `core-raster` turns one layer's contours into the 8-bit greyscale image an MSLA panel
shows. The contours come from `slicing.md`.

## From millimetres to pixels

The panel covers `width_px × pitch.x` by `height_px × pitch.y` millimetres from the plate
origin:

```
x_px = x_mm / pitch.x
y_px = y_mm / pitch.y
```

Row `0` is the near edge of the plate, because that is the first row a sliced file carries
(ADR 0134); Y is not inverted. A file asks for no mirroring at all — the profile's flags
describe the panel's mounting and reach the header only — while the app asks for `mirror_y`
to draw the plate's far edge at the top of the screen (ADR 0134). A mirror is applied here
rather than in the geometry, so the mesh displayed and the mesh sliced stay one object, and
it reverses every contour's orientation, so the rasteriser reverses a ring as it maps it
whenever exactly one of the two is set.

## The fill rule

A pixel is solid where the **winding number is positive**: walking from the left, each
crossing adds `+1` for a contour running up the image and `-1` for one running down.

Not even-odd, and the difference is not cosmetic. Two overlapping solids — a model and the
support touching it, two objects in one job — each contribute a counter-clockwise contour;
counting the winding exposes their union, even-odd would print a hole through both
(ADR 0008). Holes work because `core-slicer` winds them the other way.

Positive rather than non-zero, so that a body wound inward subtracts wherever it lands,
including outside the model — which is what makes a drain hole a mesh rather than a field
(ADR 0071). The price is that a model wound inward exposes nothing at all; both binaries
orient a mesh outward on import.

## Anti-aliasing by exact area

`Shading::Coverage` gives a pixel the grey matching the exact area the layer covers. No
sampling, no sample count.

Each edge is cut at the row boundaries. Within a row it is a straight segment, and the
area it takes from each pixel it crosses has a closed form — a triangle at either end of
the crossing, a constant slab between — deposited signed by the edge's direction. A
running sum along the row then gives the winding weighted by coverage:

```
coverage = min(|winding|, 1)
grey     = 0x00                   if coverage <= 0.02
           0xFF                   if coverage >= 0.98
           round(coverage × 255)  otherwise
```

The snapping is for the file, not the picture: a run of black or white costs one chunk
however long, so one pixel clipped by a percent splits a run into three and is paid for on
every layer. Two hundredths of a pixel is 0.4 µm on an 0.018 mm panel (ADR 0039).

Two things follow: a solid pixel between two edges receives no deposit — the running sum
carries it, so the interior is free — and the magnitude is what matters, not the sign,
because mirroring reverses every contour.

## Uniting the planes of one layer

A layer sampled at more than one plane carries the rest in `Layer::extra`. Each plane is
filled on its own and the masks combined by `LayerRuns::brightest_of`, which walks both run
lists together and keeps the brighter pixel.

Adding the planes' windings instead would be cheaper — the sweep already sums them — and
wrong: a wall covering half of a pixel on all three planes would sum to 1.5, clamp to 1,
and come out white, throwing away the anti-aliasing above. Brightness keeps a wall's edge
exactly as one plane saw it, and still lets a fin only one plane found light its own pixels.
See ADR 0114.

## The grey the panel will actually hold

Coverage can ask for any of 255 greys; a panel and a resin between them cannot hold them
all. Below roughly half the range a pixel stops curing rather than curing less, so the
resin there is left liquid behind a wall that looks printed. `Grey` is the two limits that
follow, applied after the coverage above (ADR 0113):

```
value = ladder(coverage)          rounded to `levels` rungs, or all 255
      = 0                         if value < floor
```

The ladder at `n` levels is `256/n − 1, 2·256/n − 1, …, 255`, which is what other slicers
write at anti-aliasing level `n`: eight levels are `31, 63, …, 255`. A count that does not
divide 256 lands a rung or two low, which costs nothing a printer can see.

`floor` comes from `Display::grey_floor` in the printer profile and defaults to 0, keeping
every grey: a file that prints well on the one machine measured carries the whole ramp down
to 1 (ADR 0139). A panel that leaves a dim pixel uncured asks for a floor of its own, and
pays a fraction of a pixel at every edge for it.

This is what serious font rasterisers do (`libart`, FreeType's smooth renderer,
`stb_truetype` v2, `font-rs`), except that they accumulate into a dense buffer the size of
the glyph and we deposit into the sparse rows below, so a row costs what its edges cost
rather than what the panel is wide. Both directions are exact, so error no longer follows
a shape's perimeter — a 720-gon of radius 5.03 mm is within 0.00045% of its area, and a
12 mm wedge 0.37 mm deep, nearly all near-horizontal edge, is exact (ADR 0021 replaced
ADR 0009).

The one approximation left: inside a single pixel a signed area conflates winding with
coverage, so where two unrelated boundaries cross one pixel the result is their signed sum
rather than their union. The difference is bounded by one pixel and by the mask's 8 bits.

`Shading::Binary` is for panels that ignore intermediate grey: one sweep per row centre,
and a pixel is lit when its centre falls inside a span.

## Blur

`blur_px = r` fades every edge further than coverage does: the united coverage passes
through a box `2r + 1` pixels wide, across then down, and only then through `Grey` (ADR 0117).
On a straight edge at radius 1 the pixels either side read a third and two thirds. It is off
by default: exact coverage with no floor already puts as much grey on an edge as a file that
prints well does, and a radius of 1 puts three times as much (ADR 0139).

Neither pass touches a pixel that is not near an edge. Across a row the window holds one
value unless a step lies within `r`, so only those pixels are summed and every other stretch
is its value times the window. Down a column the sum can only change where one of the
`2r + 1` rows in the window steps, so a row of output is the merge of those rows' steps.

## A layer is runs, not pixels

`ScanlineRasterizer` produces `LayerRuns`: `Run { length, value }` in reading order,
covering exactly `width_px × height_px`. It never builds the bitmap, which keeps cost
proportional to the part rather than the panel — the dark surround of a 20 mm part on a
Mars 4 Ultra is 96% of 36.8 million pixels and two entries as runs. Every format either
encodes runs directly, as `.goo` does, or expands them with `LayerRuns::to_mask`, which
only the PNG stack and the preview do (ADR 0020).

Only rows the contours reach are visited. `RunsBuilder::pad_to` fills the gap in front of
each row it is given, so a skipped row costs nothing and the dark rows merge into one run.

## Coverage as sparse differences

Within a row, coverage accumulates as `(pixel index, delta)` pairs: coverage changes by
`delta` there and holds until the next pair. A span contributes at most six pairs whatever
its length — one per partial end pixel, one opening and one closing the whole pixels
between. Emitting sorts the pairs, walks them once and turns each stretch into one run, so
a solid interior of any width is one run with no per-pixel arithmetic. It is the same
arithmetic in a different order, so results differ only in the last bits of an `f32`, well
under the rounding to 8 bits.

## The sweep

Contours become an edge table: one entry per non-horizontal segment with its Y range, X at
the top, inverse slope and direction. Horizontal segments are left out — they cannot cross
a sample line, and they would put a crossing at every vertex of a flat edge. Edges are
sorted by the top of their range and sample lines walked in one pass with an active list.
Each edge covers `[y_min, y_max)`, so a vertex shared by two edges produces one crossing,
the same half-open convention slicing uses in Z.

## What falls off the panel

Nothing here fails. A layer past the display is clipped and the distance reported in
`Rastered::overflow_px`, as `Sliced` reports what slicing papered over. The import-time fit
check works in millimetres before placement; this one catches what is about to be exposed.

## Cost

`cargo bench -p core-raster` and `-p format-goo`. Coverage now costs about 2.2 times
binary rather than 21 times, so anti-aliasing is not something to turn off for speed, and
rasterisation is no longer what a job spends its time on. Layers are independent, so a
stack parallelises across them.

## The stack

A whole stack does not fit in memory, so `encrust_cli::stack::write_stack` holds one window:
take the next `window` layers, rasterise and PNG-compress them in parallel, write them in
order, drop them. Peak memory is a window in flight — tens of kilobytes a layer as runs,
except on the PNG path, where `to_png` expands them first, which is what the bound in
ADR 0010 is still about. The window defaults to the thread count (`--raster-window`), and
compression sits inside the parallel step because deflating a panel-sized mask costs more
than the rasterisation before it.

Files are `layer_NNNN.png`, 8-bit greyscale, zero-padded so the directory sorts by name;
layer `N` is the `N`-th layer of `Sliced` whether or not it is blank. The stack is a debug
artefact, not a printer format, and lives in the binary (ADR 0011).
