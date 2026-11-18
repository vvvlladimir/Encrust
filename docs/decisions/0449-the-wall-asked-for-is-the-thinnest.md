# 0449. Cut the cavity half a voxel past the wall, so the wall asked for is the thinnest

- **Status:** Accepted
- **Date:** 2026-11-16

## Context

A wall gets three voxels of its own lattice at least (`lattice_mm`), and the cavity was cut
on the isosurface `d = -t`. Meshing moves what comes out: clustering pulls a vertex up to
half a voxel (`CLUSTER_SLACK_VOXELS`, ADR 0084) and the interpolation between lattice points
adds to it. On `niko.stl` at precision 0, a 1.3 mm wall came out 0.81 mm at its thinnest and
6.5 % of the cavity's vertices were under 90 % of the wall. On 21 closed models of the
Thingi10K corpus, a 2 mm wall at precision 0:

| | thinnest, median model | worst model | 1st percentile, median | vertices under 90 % | time | cavity |
|---|---|---|---|---|---|---|
| three voxels, cut at `t` | 0.81 t | 0.65 t | 0.87 t | 2.3 % | 2.7 s | 1.00 |
| four voxels | 0.87 t | 0.68 t | 0.92 t | 0.9 % | 3.8 s | 1.00 |
| five voxels | 0.90 t | 0.85 t | 0.94 t | 0.4 % | 4.7 s | 1.00 |
| three voxels, cut at `t + v/2` | 0.97 t | 0.78 t | 1.02 t | 0.03 % | 2.3 s | 0.90 |

A user who asks for a wall sets the thinnest a part may print with, the thickness that
survives washing and curing; one thinner than that in places is the failure, one thicker
the cost.

## Decision

The cavity is cut on `d = -(t + v/2)`, a mould's outside on `d = t + v/2`, `v` the lattice
the field is built on (`WALL_MARGIN_VOXELS` in `hollow.rs`). The lattice keeps its three
voxels a wall. Nothing else that reads the thickness moves: a drain hole, a sleeve and a
plug still measure the wall as `t`.

## Consequences

`t` is the wall at its thinnest to within a few percent: a 2 mm slab at precision 0 went
from 1.82 mm to 2.18 mm at its thinnest. The wall is thicker on average by half a voxel:
a third of a millimetre at precision 0 on a 2 mm wall, a tenth at the default precision of
0.5, and resin saved falls with it, by a tenth of the cavity at precision 0 on the corpus.
Closed-form tests of a cavity now expect `t + v/2`.

Reopen it if a coarse precision is the one users hollow at and the resin it gives back
matters more than the thinnest point, or if meshing stops moving vertices off the lattice.

## Alternatives considered

### Four or five voxels a wall

The lattice is finer, so meshing moves less; but the time grows with its square, +40 % and
+74 % on the corpus, and the thinnest point of the worst model was still 0.68 t and
0.85 t: the error is shrunk, not removed.

### A quarter voxel

Half the resin cost and half the margin; the thinnest would sit near 0.9 t, which is still
thinner than asked.

### The option that won, and what it costs

A wall thicker than asked by half a voxel on average, and less resin saved, most at a
coarse precision, where a user already accepted a rougher cavity.
