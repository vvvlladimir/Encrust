# 0294. Open a sliced file as a model, built back from its layers

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

ADR 0151 let the preview show a sliced file in place of the plate, and 0202 closed it the
moment the plate changed. A file opened that way could be looked at layer by layer and
nothing else: it never stood on the plate, the 3D view kept showing whatever model was
there, and a user who wanted to move it, print it on another machine or cut it at another
layer height had no way to. Opening it was a separate menu item from opening a model.

A file holds masks, not a mesh, so standing it on the plate means building a surface back
from them. At full resolution that is tens of gigavoxels for one 16K plate, so it has to be
streamed and kept sparse (rule 7). `core-volume` already holds a sparse narrow-band field
and the surface-nets extraction every cavity goes through.

## Decision

A sliced file opens as a model, from Open model and from a drop, in the same import job
as a mesh. `core_volume::StackField` takes its layers bottom first as `(length, grey)` runs
over a `Panel`, spreads each run over voxel cells by pixel centre, and keeps the cover of
two cell slabs and one tile row of lattice planes at a time, each only over the cells a
layer lights. A lattice point's value is `(0.5 − cover) · 2v` for the mean cover of the
eight cells around it, so a surface never passes between two saturated points; a tile all
lit is solid, all dark is not stored. `extract` turns it into a closed mesh. The lattice is
the layer height or 0.05 mm, whichever is coarser, and never finer than a pixel.

The window places the panel from the printer it was cut for — the catalogue's one of its
panel, or the machine in hand of the same panel — whose pixel size and mirroring it takes,
or from the file's own panel size with no mirroring. Mirroring is taken back out, so the
model is the one the file was cut from. The preview's file source, its menu items and the
opened-file inspector are gone: Preview always shows the plate. `.cbddlp`/`.photon` now
read the panel size the header's bed fields carry, as every writer of the family fills them.

## Consequences

The model can be moved, scaled, supported again and cut for any printer. What the file was
printed with comes back with it: its supports and raft are part of the mesh, its edges are
the voxel's rather than the original model's, and its layers' steps stay. A 20 mm cube of
571 layers on a 16K panel opens in about a second, a `.sl1` of the same in over a minute,
nearly all of it decoding the PNG layers. Looking at a known file's exact masks is no
longer done in the window; `encrust info` still decodes and checks every layer. Reopen if a
user needs the file's own masks on screen.

## Alternatives considered

### Faces of the voxels, merged into rectangles

Exact to the pixel, with three layers held at a time. It leaves T-junctions where merged
rectangles meet, which is not a closed surface the slicer can cut again, and it adds a
second extraction beside the one `core-volume` already proves.

### Keep the file as a stack that is only written out again

No loss at all, but nothing on the plate to move or support, which is what was asked for.

### The option that won, and what it costs

A box filter two voxels wide rounds every edge by about a voxel, and detail finer than
0.05 mm is smoothed away. The lattice planes are held over the lit cells' bounding box, so
a plate lit at both far corners costs a plane the size of the panel.
