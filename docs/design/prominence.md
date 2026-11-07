# Prominence

How much each face of a model shows on the finished print, from 0 for a face hidden in a
fold to 1 for open surface with fine detail: where a support's scar costs the most.
`core_geometry::Prominence` reads it once a model. The clean surface goal weighs supports by
it (`orientation.md`, ADR 0409).

It follows Zhang et al., *Perceptual models of preference in 3D printing direction*
(SIGGRAPH Asia 2015). They score a print direction on the supports' area, the visual
saliency under them, how visible they are from preferred viewpoints, and smoothness.
Encrust keeps two of those readings and multiplies them, one value a face:

```
prominence = exposure × (1/3 + 2/3 × detail)
```

The third is what an open face shows with no detail at all: a scar on smooth surface still
shows, only less.

## The cells

Everything is read on a sparse grid of cubes, not face by face, so the cost follows the
surface and not how finely it is meshed. The finest cube is 0.6 % of the model's diagonal,
twice the 0.3 % Lee et al. take as their smallest scale. Six levels each double it, up to
32 times the finest. A face belongs to the cube its centroid falls in. A mesh drawn in
few large faces, as CAD exports are, is read at its centroids only: its prominence is
coarse, and for such a part that matters little.

## Detail: mesh saliency

After Lee, Varshney and Jacobs, *Mesh saliency* (SIGGRAPH 2005):

1. **Curvature.** Each edge shared by two faces bends by the angle between their normals,
   positive where the surface bulges and negative where it folds in. Half that angle times
   the edge's length is the edge's integrated mean curvature, split between its two faces.
   A cube sums its faces' curvature and area.
2. **Smoothing.** At each level, the mean curvature around a finest cube's centroid is read
   from the 27 cubes around it. Each counts by its area and by `(1 − d / 2h)²`, with `d` how
   far its own centroid lies and `h` the level's cube. A box of cubes with that falloff
   stands in for Lee's Gaussian at a cost of 27 lookups a level.
3. **Centre-surround.** Each level minus the next coarser is a difference map: large where
   the curvature changes at that scale, which is detail, and small on a uniform curve, where
   it does not.
4. **Normalising.** Each map is divided by the value its top twentieth of cubes reach and
   clipped at 1, or divided by three over the model's diagonal if that is larger, so that a
   smooth model's noise is not stretched to fill 0 to 1. Lee divide by the largest and
   suppress maps with many peaks (Itti, Koch and Niebur). On a sculpted figure, one sharp
   tip read 20 to 60 times the median, and the face came out near nothing; on a figure
   detailed all over, every map has many peaks, and the suppression flattened them all.
5. The five maps are averaged.

## Exposure: how open the surface is

Ambient occlusion: the share of the sky over a face that reaches past the model, weighted
by the cosine, as light falls. It is read at the second level, one cube twice the finest, and
apart for each of the six axis directions a face is nearest. That way both sides of a wall
thinner than a cube are read apart. In each such cube the largest face is looked out from:
from its centroid, a hundred-thousandth of the diagonal off the face, the half of 64
Fibonacci directions it faces is cast through the `Bvh`. A ray that meets nothing escapes.

A slot 2 mm wide and 18 mm deep lets `sin(atan(1/18)) = 0.055` of the sky onto its floor,
and the test holds it to that.

## Cost

About 0.3 s for a million triangles (`cargo bench -p core-geometry --bench prominence`),
most of it the rays. It is read only where the clean surface is asked for: once in the
search, once in the proof, and once a pose in `print_out`.

## What it does not know

Which way the model is looked at. Zhang et al. learn preferred viewpoints. A model's file
does not say which way is up, and ADR 0284 dropped the file's +Z as a stand-in. So exposure
treats every open direction alike. Painting a face to keep clean would answer what geometry
cannot: `TODO(step-2)` of ADR 0409.
