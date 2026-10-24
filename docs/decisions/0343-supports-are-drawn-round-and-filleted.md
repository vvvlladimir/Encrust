# 0343. Draw supports with 24 sides and round joints, and fillet a pillar into its pad

- **Status:** Accepted; 24 sides are superseded by 0352, the flare widths by 0355
- **Date:** 2026-10-24

## Context

Next to other slicers ours looked like pencils: twelve sides to a pillar, joints closed
by domes of three rings, which at a bend show outside the strut as a coarse lip, and a
pad met by a straight 45-degree cone (ADR 0130) — or by nothing, since Light, Medium and
Heavy set the flare as wide as the pillar. A notch where a pillar meets its pad is where
the peel starts a crack.

## Decision

The default `facets` and every shipped preset go from 12 to 24. A dome is drawn with six
rings. The flare is a quarter circle tangent to the pillar and to the pad, in four steps,
still rising as far as it widens and never wider than the 45-degree cone it replaces.
Light, Medium and Heavy get a flare: `lower_diameter_mm` 2.0, 2.4 and 3.6 against pillars
of 1.0, 1.3 and 3.0, Heavy's held to what its 5 mm lift leaves room for.

## Consequences

Supports read as round on the screen and the pillar's foot carries no step. A pillar has
twice the faces, so the support mesh and its slicing cost more: `mesh_trees` on the lifted
40 mm ball went from 0.63 ms to 1.33 ms, and its supports from 1738 to 1842 mm³ as the
polygons come nearer their circles and the fillets fill in. A profile without `facets` now
draws 24 sides. Smooth shading is not part of this: the viewport lights every mesh flat,
and changing that is the renderer's.

## Alternatives considered

### A fillet height of its own, as a profile field

Another number to tune for a shape the flare already describes; the flare's width sets it.

### Smooth normals for the supports

Would hide the facets at any count, but needs a second pipeline or per-vertex normals in
`encrust-app`, a change to every mesh drawn.

### The option that won, and what it costs

Heavier meshes, a retune the presets' users will see in the size of a pad's foot, and a
shape the volume tests now compute step by step instead of as one frustum.
