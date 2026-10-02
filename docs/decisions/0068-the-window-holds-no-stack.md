# 0068. The window holds no stack: it cuts the window it is showing

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

The preview used to keep the whole stack as contours and rasterise the layer being looked
at, and the exporter used to reuse that stack rather than cut the plate twice. Both were
decided when a stack was small.

They are not any more. A dense infill puts tens of thousands of contours on a layer, and
the measurement behind ADR 0066 put a 60 mm ball's stack at over a gigabyte — held for as
long as the Preview tab is open, on top of the meshes, the hierarchies and the GPU
buffers. The CLI now streams and holds nothing; the window was the last place a whole
stack lived.

Keeping the stack as compressed layers instead was the obvious swap and does not work: the
file the layers compress into is 817 MB on that plate, and run-length data is no smaller
than the contours it came from on an ordinary model.

## Decision

The window keeps no stack at all.

A preview build merges the plate and works out its layer heights — that is all, and it is
instant. The panel shows a layer by cutting the window of 64 layers it falls in and
keeping that one window; stepping inside it costs nothing, and crossing into the next one
cuts again. Layer count and Z come from the heights, so the slider and the rail are
answered without cutting anything.

Export cuts the same way: window, rasterise, write, drop, with the volume laid into the
header at the end (ADR 0067). There is nothing to hand over from the preview, so the
handover goes with it.

## Consequences

The window's memory no longer follows the plate's complexity: one window of contours,
whatever the model. The plates that used to kill it now open.

The Preview tab is ready as soon as the plate is merged rather than after a full slice,
and the first frame on each window costs one window's cut. That cut rebuilds the face
index, which walks every face, so crossing a window boundary on a large plate is a visible
hitch — the transport stepping through the stack hits it every 64 layers.

Exporting after previewing cuts the plate again. That is the cost the old handover was
written to avoid, and it is now paid deliberately: the stack it saved is what did not fit.

The exposed area of a layer is only known once its window has been cut, so the reading is
blank for the frame before the picture appears.

`merge_visible` still copies every model into one mesh for both jobs. That copy is the
next thing to remove, and it is not this decision.

## Alternatives considered

### Keep the stack, but as compressed layers

Measured and rejected above: no smaller in the ordinary case, still hundreds of megabytes
in the bad one, and it ties the stack to one panel and one shading mode.

### Keep a window of contours around the slider, prefetching neighbours

Smoother scrubbing. Rejected for now as more machinery than the hitch justifies; the
prefetch can be added behind the same interface if the hitch is felt.

### The decision above, and what it costs

Scrubbing is no longer free: every 64 layers the window cuts, and on a heavy plate that is
a fraction of a second where the old design had already paid for everything up front. The
work is also done twice when the user previews and then exports, where it used to be done
once. Both are memory bought with time, deliberately.
