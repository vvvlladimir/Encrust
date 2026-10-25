# 0350. Repair stitches a seam before it patches

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

A file exported from patches meshed at different densities has seams: one side's open edge
with the other side's vertices standing on it. Repair (ADR 0270, 0349) read every seam as a
hole and patched it. Patching a crack a micron wide lays a strip of slivers along it, and
the projected ear clipping a long loop falls back to could leave faces with no area there.

On the test models, two of the five open ones have seams: a statuette with 21 open edges and
a 4-million-face colour mix with 2 577. Measured with `encrust inspect --repair`:

| Tolerance | Statuette | Colour mix |
|---|---|---|
| none (patch only) | 7 holes patched | 7 holes patched |
| 0.001 mm | 6 seams stitched, 1 hole patched | 549 stitched, 5 patched |
| 0.01 mm | 7 seams stitched, nothing patched | 1 887 stitched, 6 patched |
| 0.1 mm | 7 seams stitched, nothing patched | 1 917 stitched, 9 patched |

Stitching costs nothing measurable: the colour mix repairs in 4.4 s stitched and 4.9 s patched.

## Decision

**`core-geometry::stitch_seams` closes a seam by fanning the coarse side's face through the
fine side's vertices**, and `mend` runs it after dropping doubled faces and before the first
orientation. No vertex is made or moved. A vertex is laid into an open edge only when it
stands strictly between the edge's ends, within the tolerance of it, and ends an open edge
walked the other way along the same line; a split that would leave a face with no area is
skipped. The search is a grid over the open corners; see `docs/design/mesh-repair.md`.

**The tolerance is 0.01 mm by default** (`DEFAULT_SEAM_TOLERANCE`), and the command line
sets it with `--seam-tolerance`. The window uses the default until the Repair tool gives it a
field.

**Ear clipping a loop refuses a result holding a face with no area**, so the loop is fanned
instead. The corpus showed stitching alone left such faces after the fill; they came from
corners in a row that the projection rounded into an ear, which can happen on any loop.

## Consequences

- A seam closes along the surface it belongs to, with no new surface across it.
- Repair reports seams stitched beside holes closed, in the stage notice, the text report
  and the JSON `seams_stitched`.
- On the colour mix the stitched model has three more branching edges than the patched one:
  bodies that touch along a seam are sewn together. They still close.
- The signal to revisit is a model where a crack wider than 0.01 mm is a seam, or a narrow
  slot that is not one gets sewn shut.

## Alternatives considered

### 0.001 mm, the tolerance a quantising repair would use

Closer to the weld. Rejected because it left a hole patched on the statuette and two thirds of
the colour mix's seams unstitched: their vertices stand a few microns off the edge.

### Any vertex within the tolerance of an open edge

The simplest rule. Rejected because it sews a sheet merely standing on an edge into it.

### The option that won, and what it costs

A pass over the open edges on every repair, and a fixed tolerance that is a length, not a
fraction of the model: a model drawn at a very small scale can have a real slot under it.
