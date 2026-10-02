# 0057. A field is band tiles and solid runs, on one global lattice

- **Status:** Accepted
- **Date:** 2026-09-21

## Context

A 100 mm model at 0.1 mm is 10⁹ voxels. Dense storage is four gigabytes for one model and
is not on the table; a three-voxel band around the surface is of the order of 10⁷ values,
which is 40 MB and comfortable. So the field is sparse, and the question is what it says
about the space it does not store.

Extraction only needs the band: the surface is where the field crosses zero. Everything
else needs more. Subtracting a drain hole from a solid asks for the big field's value at
the small one's surface, which is deep inside it, far past any band. If unstored space
answers "outside", the difference keeps the whole solid and the hole never appears.

So a field has to know its own interior, without storing it. And two fields have to be
combinable at all: a field built from each of two meshes lands on whatever lattice its own
bounding box suggested, and resampling one onto the other's lattice to subtract them would
lose a voxel of accuracy for nothing.

## Decision

An `Sdf` holds two things:

- **Band tiles.** 8³ voxels, 512 `f32`, in a hash map keyed by tile coordinate. A tile is
  stored only if some voxel of it carries a real distance rather than the clamped band.
- **Solid runs.** Per tile column, the inclusive ranges of tile Z that lie wholly inside
  the surface. A voxel in no stored tile answers `-band` if its tile is in a run and
  `+band` otherwise.

The runs are found from the band tiles themselves. Within one column, a stretch between two
stored tiles is either wholly inside or wholly outside, because a surface crossing it would
have left a stored tile behind; so each gap is decided by one question asked at its lowest
tile. Below the lowest stored tile and above the highest, a column is outside.

The lattice is global: voxel `(0, 0, 0)` is the origin of the space the mesh is in, and a
`VoxelGrid` is nothing but a spacing. Two fields of the same spacing therefore share a
lattice whatever they are fields of, and the CSG operators only have to check that the
spacings match.

## Consequences

The 200k-triangle, 40 mm sphere at 0.1 mm stores 16 259 tiles — 8.3 million values, 33 MB —
for a body whose dense field would be 64 million voxels. Its interior costs a handful of
run entries.

`difference` of two balls comes back as a shell with two closed surfaces and the analytic
volume, which is the test that the interior sign is really there.

A field carries no origin, so it cannot be translated: moving a model means rebuilding its
field, not shifting it. That is the right way round for this pipeline — a field is built
from a mesh that has already been placed — but it does mean an interactive drag cannot
reuse one.

Tile granularity is also a cost. Building fills all 512 voxels of every tile the band
touches, and at 0.1 mm with a 0.3 mm band that is roughly twice the voxels the band
actually needs. A row skips ahead by what the distance it just found guarantees, which
recovers most of it: 2.21 s down to 1.31 s for that sphere.

The runs assume a column's gaps are uniform, which follows from the band being at least one
voxel wide — which is why a narrower band is refused rather than clamped.

## Alternatives considered

### Uniform tiles in the same map

Store interior tiles as a single value each, the way OpenVDB does. It answers the interior
question with no second structure. A 100 mm model at 0.1 mm is 1.95 million tiles inside
its own bounding box, so the map grows by two orders of magnitude to hold numbers that are
all the same.

### No interior at all, and let the caller cope

Simplest, and enough for step 8c's own milestone: extraction never asks. It makes
`difference` quietly wrong in exactly the case step 8e needs it for, and a field that lies
outside its band is worse than one that refuses.

### A per-field origin, with resampling to combine

What the first cut did: each field's lattice starts at its own bounding box. It keeps tile
coordinates small and near zero, and it makes every binary operator either an error or a
resampling. The global lattice costs nothing but larger integers.

### The option that won, and what it costs

Two representations of the same field, which every operator has to maintain: the CSG
operators rebuild the runs from the result's own tiles rather than combining the inputs'
runs, which is simple but means a full walk of every column. The interior is also only as
good as the gap rule — a band narrower than a voxel, or a surface that slips between two
lattice points, would put a solid where there is none, and nothing downstream would catch
it.
