# Distance and inside-ness queries

How far the nearest surface is, and which side of it a point is on.
`core-geometry/src/closest.rs` and `winding.rs`, both walking a tree beside `bvh.rs`.
Why they live there: ADR 0053, 0054. `core-volume` asks them of every voxel in a band.

They are deliberately separate calls: the distance is exact and cheap, the sign is
approximate and is the expensive half.

## The nearest point on a triangle

`point_triangle` uses the Voronoi-region test of Ericson, *Real-Time Collision Detection*
§5.1.5: the plane divides space into seven regions — one per vertex, one per edge, one for
the face — walked in an order where each step reuses the last one's dot products. With
`ab = b - a`, `ac = c - a`:

```
d1 = ab·(p-a)   d2 = ac·(p-a)
d3 = ab·(p-b)   d4 = ac·(p-b)
d5 = ab·(p-c)   d6 = ac·(p-c)
```

Vertex regions are `d1 ≤ 0 ∧ d2 ≤ 0` at `a`, `d3 ≥ 0 ∧ d4 ≤ d3` at `b`,
`d6 ≥ 0 ∧ d5 ≤ d6` at `c`. The edge regions come from the barycentric determinants
`d1·d4 - d3·d2` and its rotations, each paired with the dot products saying the projection
lands between the edge's ends. What falls through is inside the face, and those
determinants are its barycentrics.

A face with no area has all three determinants zero. The vertex and edge tests catch the
ordinary collapses but are not guaranteed to catch every one, so the face branch checks
their sum before dividing and falls back to the nearest vertex. `Bvh::build` keeps
degenerate faces on purpose — dropping them would shift every face index — so this branch
has to hold.

## Walking the hierarchy

`Bvh::closest` is `Bvh::raycast` with a different cull: a node is worth opening while the
squared distance from the point to its box is under the best found so far, with the usual
clamp

```
outside = max(mins - p, p - maxs, 0)
distance² = |outside|²
```

which is zero inside the box. Squared throughout, so no square root per node.

Both children are measured and the nearer pushed last, so it opens first and its bound
culls the sibling. Each stack entry carries the distance its box was at when pushed, so a
node the search has outgrown is dropped without measuring again — worth 36% of the query.
The traversal is best-first in spirit without a priority queue, which on this tree costs
more than it saves. The stack is a fixed array rather than a `Vec`, because a field build
asks millions of these and one allocation each is measurable; it holds one slot more than
`MAX_DEPTH`, since popping the deepest internal node leaves `MAX_DEPTH - 1` entries and
pushes two children over them.

## The sign

A closed surface wound outward subtends `4π` at every interior point and `0` outside.
Over `4π` that is the winding number: one inside, zero outside, and — the point — a
fraction near a hole or a self-intersection, rather than a sign that flips over a whole
region the way ray parity does.

`solid_angle` is Van Oosterom and Strackee's formula, with `a`, `b`, `c` relative to the
query point:

```
Ω = 2·atan2( a·(b×c),  |a||b||c| + (a·b)|c| + (b·c)|a| + (c·a)|b| )
```

signed by the winding, so an inward-wound mesh answers `-1` inside. A point exactly on a
vertex makes both arguments zero and `atan2` returns zero — the only place the formula has
nothing to say, and the one place the answer does not matter, since the distance there is
zero whatever sign it carries.

## Summarising distant faces

Summing every face costs milliseconds, so `Winding` groups them after Barill et al.,
*Fast Winding Numbers for Soups and Clouds* (2018).

The tree is `Bvh`'s median split with eight faces to a leaf rather than four, since a leaf
here is usually summarised rather than walked. Each node carries `center` (area-weighted
centroid), `radius` (`max(|centroid_i - center| + radius_i)`, an upper bound from per-face
numbers, so the build never looks at a triangle twice) and `dipole` (the sum of face
normals scaled by area).

A patch of area `A` and unit normal `n̂` at distance `d` subtends `A·(d·n̂)/|d|³` — a
dipole field — so for a node whose faces sit near its centre

```
Ω ≈ (center - p) · dipole / |center - p|³
```

is one dot product for however many faces it holds. It is used when the point stands off
by more than `FAR_ENOUGH` — Barill's beta, 2 here — times the node's radius; anything
nearer is opened, and a near leaf is summed exactly.

## What that costs

`core-geometry/benches/field_queries.rs`, points in a band around the surface, which is
where a field is evaluated and nowhere else. The nearest point is microseconds and the
winding number is an order of magnitude more, for a reason that is not an implementation
detail: a band point is close to the surface, which is exactly where nodes cannot be
summarised. The nearest-point search culls those nodes as soon as it has a tight bound;
the winding number has to add up everything it cannot summarise, and near the surface that
is thousands of triangles.

The dipole truncation error falls as the square of the stand-off while cost roughly
doubles with it: on a 20 mm sphere the worst error is 3.6% of a winding at beta 2, 1.1% at
3, 0.2% at 6. Three percent is nothing against the half-winding the sign is read at, and
far too much for anyone wanting the winding number itself — the lever there is the
quadrupole term, not a wider beta.
