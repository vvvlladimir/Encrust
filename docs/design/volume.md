# The volume engine

How a mesh becomes a signed distance field, what the operators do to it, and how it comes
back a mesh. `core-volume`: `grid.rs` and `sdf.rs` the lattice and storage, `build.rs` the
walk and the assembly, `scatter.rs` the seed, `sweep.rs` the fill, `sign.rs` the side,
`csg.rs` the combinations, `extract.rs` surface nets. The queries under it are in
`distance-queries.md`; what is built on it is in `hollowing.md`.

Why: ADR 0055 (two sign paths), 0056 (generated table), 0057 (storage), 0058 (band on an
isosurface), 0063 (priced before filled), 0064 (merge by lattice edge), 0065 (quantised
values), 0069 (the walk), 0082 (scattered and carried coarse), 0084 (clustered surface
nets).

## The lattice

A `VoxelGrid` is one number, the spacing. Voxel `(0,0,0)` is the origin of the space, so a
voxel's position is `voxel * spacing`. Two fields of one spacing therefore share a lattice
whatever they were built from, and the CSG operators work voxel by voxel with no
resampling. A voxel is a lattice *point*: `value` answers at one, `sample` interpolates
trilinearly between the eight around a position.

## What is stored

Tiles of 8³ voxels in a hash map keyed by tile coordinate, kept only where the band
crosses. A tile the band misses holds the band's value repeated, which is cheaper to say
than to store.

Distances are millimetres, negative inside, clamped to the band; past the band the field
knows only the sign. A stored value is an `i16` step of the field's own band rather than an
`f32`, so a tile is a kilobyte and reading is a multiply — on a two-voxel band at 0.1 mm a
step is six nanometres (ADR 0065).

What is deep inside is kept as runs: tile columns keyed by `(x, y)`, each carrying the
inclusive ranges of tile Z wholly inside. Within a column, a gap between two stored tiles
is wholly inside or wholly outside — a surface crossing it would have left a stored tile —
so one question per gap decides it, and a column has a handful of gaps.

## Finding the tiles

The isosurface stands a wall thickness from the mesh, so the tiles to fill are not the
ones the mesh lands in. They are found by walking outward from those, ring of neighbours
by ring of neighbours, while a tile is still nearer the mesh than the isosurface's band —
the shell between the two, not the ball around them, which for a thick wall is thousands
of times fewer tiles. One nearest-point query per tile, which the fill needed anyway. See
ADR 0069.

## What it may cost

The tiles a build will fill and the tiles its carrier will hold are both known before any
is filled, so `build` prices the larger of its two phases — filling, which holds the
carrier, and meshing, which holds the cavity — and refuses a `FieldSettings::budget_bytes`
it cannot meet with `VolumeError::TooFine`, naming the spacing that would have fit
(ADR 0085). Hollowing catches that and retries coarser. `FieldSettings::clip`
bounds a build to part of space; a clipped column gets a sentinel tile past each end, so
what the clip cut off still reads as solid (ADR 0063).

## Building

A field is built around a surface, not a mesh: `FieldSettings::iso_mm` picks it — zero the
mesh, negative an inward offset, positive an outward one — and the band follows, so a wall
of any thickness costs what the mesh's own field costs (ADR 0058).

1. Refuse an empty mesh, a zero voxel, a band narrower than a voxel, an infinite
   isosurface. The band rule matters: under one voxel the surface can pass between two
   lattice points unnoticed, losing a tile and with it the run rule.
2. Collect candidate tiles — every tile a face's bounds reach, grown by the band, on every
   core — then walk out to the isosurface from them, one ring at a time, stepping on only
   while a tile is still nearer the mesh than the isosurface's own band (ADR 0069). Behind
   the mesh, away from its own isosurface, the walk stops at the tiles the surface
   straddles. What comes back is the shell in the rings it was found in, and the subset of
   it the band crosses.
3. Price that subset and refuse a budget it cannot meet (ADR 0063).
4. Seed, carry and refine — below.
5. Recompute the solid runs from the surviving tiles. A tile is inside when its centre's
   signed distance is below the isolevel.

### Seed, carry, refine

Nothing asks the hierarchy per voxel, and nothing measures the wall's interior at the
resolution the band is stored on (ADR 0082).

**Seed.** `scatter.rs` buckets each face into the tiles its bounds grown by `SEED_VOXELS`
reach, sorts the pairs by tile and fills each tile from its own faces alone. A voxel keeps
a face only within that reach, where the list is complete, so the seed is exact. Work
follows the surface, not the volume around it: a face is measured against the voxels its
own bounds cover and no others.

**Carry.** `sweep.rs` propagates the nearest *face* outward from the seed to the
isosurface, on a lattice `CARRY` times coarser than the stored one — crossing a wall is a
volume, and crossing it coarse is what makes it affordable. One tile is swept in a block of
its own voxels plus a one-voxel halo, eight directions over, each voxel taking whichever of
the seven faces upstream of it stands nearest: Zhao's sweep carrying a closest primitive
rather than a value. Seven and not three, or a face waiting in a corner of the block could
never step in. Tiles come in the walk's rings, and the whole shell is then swept once more,
because a tile only sees the faces already in its block and one can be stranded in a
neighbour of the same ring; that second pass skips a tile whose halo offers no voxel on its
border a nearer face, since nothing else can move it. Only two rings are kept as halo.

The carrier is `FAR_CARRY` = 4 times coarser instead where the whole band lies past what
that carrier is trusted for, `3 · 4 · voxel < |iso| - band` — a 2 mm wall from about
0.14 mm down. That is an eighth of the sweep again; where the band reaches into the
trusted distance, every voxel of it would fall back on the hierarchy and cost more than the
carrier saved, so the factor stays 2 (`carry_for`). On a 406k-face bust at 0.078 mm around a
2 mm wall the field fell from 13.4 s to 6.2 s and its peak from 633 MB to 370 MB, with the
wall no further off than before.

**Refine.** A stored tile takes, per voxel, the nearest of the faces the eight carrier
points around it hold. The `CARRY³` voxels of a carrier cell share those eight, so they are
named once per cell and there are only ever two or three distinct ones. Nearer the mesh
than `TRUST_VOXELS` carrier steps the carrier is not believed — the lateral miss costs
about `step² / 2d` and that blows up as `d` falls — and the hierarchy is asked for that
voxel, but only where the value would not be clamped to the band anyway. It is asked
`closest_within` the carried face's own distance: only a nearer face can win, so the
search culls from its first node and still comes back exact (ADR 0309).

Neither the distance nor the side is carried with the face. The distance is a
`point_triangle` away, and four more bytes a voxel over a shell is not affordable. The side
cannot be carried at all: both sides of a face are upstream of some voxel and the sweep
cannot tell them apart, which inverted 228 voxels on a ball before it was found.

## The sign

`SignMode::Auto` runs `diagnose` once. A closed mesh is signed by the angle-weighted
pseudonormal of Bærentzen and Aanæs — the nearest point's barycentrics choose the face,
edge or vertex normal, and the sign is its dot with the offset. A mesh with a hole is
signed by the generalised winding number, which degrades gracefully where the pseudonormal
inverts (ADR 0055).

It is asked for only where values are written, once per 4³ block of a stored tile: a block
the surface crosses no lattice edge of is all one side, and a band sitting a wall off the
mesh is never nearer than that. That is what keeps the winding number affordable. The one
question is asked at the true nearest point, not the carried face: near a thin wall the
carrier hands over the wall's far side, close enough for a distance and wrong for a side.

Most tiles are not asked at all. The walk already knows the signed distance at each tile's
centre, and no surface is nearer a point than its distance, so a tile whose centre stands
further from the mesh than its own radius takes that side for every voxel; the walk passes
a side on to its neighbours the same way. Only near a hole of an open mesh, where the
winding number can cross one half without crossing a face, can this differ from asking.

The pseudonormal tells a seam from a face by barycentrics, except on a triangle whose angle
at its first corner is under about six degrees, where `f32` cancellation loses the seam and
can leave the determinant at zero or below. There the offset decides: straight along the normal is the face, otherwise the nearest corner or
edge (ADR 0186).

## The operators

`union` is `min`, `intersection` is `max`, `difference` is `max(left, -right)`. Binary
operators require one spacing and answer `GridMismatch` otherwise. `offset(d)` subtracts
`d` from every value; `shell(t)` is `max(v, -(v + t))`, the wall between the surface and
its inward offset. All of them walk the union of the tile sets on every core and keep only
the tiles the result's surface crosses, then recompute the runs by the build's gap rule —
a shelled solid ends up with no runs, which is correct, its middle is hollow.

Offsetting and shelling narrow the band by what they moved, because the field cannot speak
about a surface past its own reach. Asking for more is `OutsideBand`, never a clamp.

### Pressing a texture in

`press` is an operator with a query behind it: at every lattice point of the band, the
nearest point of the mesh names a face, the `UvMap` gives that point's coordinate and which
of the model's images it lands on (ADR 0123), and that `Heightmap` gives a height which is
subtracted like an offset — one whose distance varies over the surface. The band is built
around the middle of the range the surface can move over, as an iso level, and as wide as
half that range on top of what marching needs, so the moved surface stays in the tiles the
field stored. The height is subtracted as each value is built (`build_moved`), at the
nearest point the build asked the hierarchy for anyway, rather than by a second query per
lattice point afterwards (ADR 0309).

The relief is only as fine as the lattice, whatever the image's resolution, and only as
continuous as the map: a UV that jumps between two faces jumps the surface with it, and so
does the edge of a map that covers only part of the mesh (ADR 0121). The amplitude is a
plate millimetre, so a caller presses after placement or divides by the model's own scale,
and `press` reports the lattice it settled on (ADR 0122). See ADR 0116.

A picture with no map behind it is laid on by projection instead (`press_tiled`): it is
read as square tiles of `tile_mm`, turned by `angle_deg`, on each of the three planes the
model's own axes span, and the three readings are blended by the nearest face's normal
raised to the fourth power, so a face takes the plane it squarely faces and a 45° face
takes two equally. Its axes are the model's own, at the size it prints at (ADR 0225).

Both read the image through `prepared`: blurred by `smoothing_px` with three box passes of
radius `r`, `r(r + 1) = σ²`, wrapping at the edges because a texture tiles, then turned over
if `invert` asks. `symmetric` reads each height less one half, so mid grey stays put and
the relief spans half the amplitude either way (ADR 0307).

An `AngleMask` holds the faces within `bottom_deg` of `-up` or `top_deg` of `up`, in a
hierarchy of their own. A lattice point whose nearest held point is as near as its nearest
point on the model, within 1e-4 mm, keeps its value, so an edge between a held and a moved
face goes to the held one. That alone keeps a relief off a held face's square edge, but not
off one a face meets leaning down — a chamfer, the underside of a sphere — whose relief
would hang below the plane. So a point outside the model and past the held face's edge,
off its normal, takes the larger of its moved value and its distance above the held plane:
the moved solid intersected with the half-space behind that face, on both sides of the
plane so the lattice interpolates the cut onto it. A point straight over a held face is not
cut, because it lies in the open under an overhang beside a wall that keeps its relief.

## Extraction

Surface nets over the band, one tile at a time on every core, clustered onto the lattice.
A tile reads a 9³ block — its own values plus the neighbour row its far cells reach into —
with one lookup for each of the eight tiles the block spans, so a cell costs no hash
lookups.

A cell the surface passes through gets one vertex, the mean of the crossings of its own
twelve edges, and the gradient of its trilinear field as a normal. A crossed lattice edge
gets one quad joining the four cells around it, claimed by the cell that owns the edge, and
wound by the sign at its near end. Marching cubes' 256-pattern table is gone with it.

That alone is not fewer triangles — a smooth surface crosses half again as many edges as
cells, which cancels the saving exactly. The reduction is the clustering: a tile is walked
as its own halves, quarters and eighths, and a block becomes one vertex when every cell of
it stands within half a voxel of their shared plane and faces the same way. A flat stretch
comes out as a few facets and a tight one keeps its cells, so the count follows the cavity's
shape rather than the spacing. Blocks never cross into another tile, so no tile has to agree
with any other about where a facet ends.

Two things the curve does that have to be undone. The mean of a curved patch lies inside
the surface, so a facet is lifted back out along its normal by the rise the cells' own
normals imply. And clustering can fold two quads onto one triangle. Copies are summed the
way the surface sums them: one wound against another takes it out, and two wound alike both
stay. Dropping either kind alone opens the surface. See ADR 0084 and 0186.

A quantised value can be exactly zero, and every cell around a lattice point at zero
crosses the surface there, so their vertices are one point, or a rounding of an `f32` apart.
A triangle between two of them has no area, and a mesh with one reads as broken. The
corners of such a triangle within `1e-4` mm are welded, which collapses the zero-length
edge the way a decimator would and keeps the surface closed (`collapse_flat_faces`,
ADR 0309).

A quad around an edge on a tile's lowest face names cells of the tiles below it, which the
field need not store when the surface only crosses the face they share. Those tiles are
meshed too, and the ones below them in turn, so no quad loses a corner and the cavity comes
out closed whatever the field stored (ADR 0186).

Tiles are never welded. Every vertex is named by the cell it stands in — that cell packed
into a `u64` — so a cell clustered with others simply names the same vertex from each.
Tiles are merged a layer of tile Z at a time in ascending X and Y, which is also the order
a quad reaches back in, and both tables are cut back to the plane between layers (ADR
0064). One copy of the surface is resident rather than three.

## A field from a stack of layers

`StackField` builds the field back from a sliced file's masks (ADR 0294), a layer at a
time. Each pixel goes to the voxel cell its centre falls in; a cell's cover is the lit
fraction of its pixels, and a layer adds to the slab of cells it overlaps by the share of
the slab it fills. A lattice point takes the mean cover of the eight cells around it, two
slabs by two by two, which is a box filter two voxels wide: a flat boundary makes the cover
fall linearly with distance, so `(0.5 − cover) · 2v` is the signed distance within a voxel
of the surface and saturates beyond it. Every crossing therefore has an unsaturated point
beside it, whose tile is stored, and the extraction's growth reaches the rest.

Only two slabs, one tile row of planes and the lit bounding box of each are held; a tile
row is turned into tiles on every core and dropped. Runs are spread a cell row per core.

## What it costs

`cargo bench -p core-volume`, whose last case is a model the size of a real one.
On an 818k-face ball 150 mm across, at 0.2 mm around a 2 mm wall: 5.3 s to build, of which
the carrier is the larger half and the refine most of the rest. The walk, the seed and the
runs together are under a fifth.

What the field promises is half a voxel against what the hierarchy would answer, over the
whole band, at `iso = 0` and at a wall's offset. `the_sweep_answers_what_the_hierarchy_would`
is that promise, on both carriers; `CARRY`, `FAR_CARRY`, `TRUST_VOXELS` and the eight sweep
directions are each set by it and not by argument.

Extraction of that same ball's cavity is 0.3 s for 314 000 triangles, where marching cubes
gave 5.0 million for the same surface. `a_smooth_cavity_costs_far_fewer_triangles_than_the_cells_it_crosses`
holds both ends of that: the count, and that what comes out is still closed.
