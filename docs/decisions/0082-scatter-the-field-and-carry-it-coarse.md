# 0082. Scatter the field from the faces, and carry it across the wall coarse

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

`build` filled a tile by asking the `Bvh` what stood near it: one traversal and one
`faces_within` per 4³ block, then every voxel of the block measured against that list.

For hollowing the band sits at `iso = -t`, a whole wall thickness off the mesh (ADR 0058),
and the gathered list is a ball of radius `d + 2r` around the block. Its intersection with
the surface has area about `4π r d`, so **the list grows linearly with the block's own
size and with the wall thickness**: on an 818k-face ball, 150 mm across, at 0.2 mm and a
2 mm wall, that is ~190 faces measured against each of 27.5 M voxels. Measured with
`big_model_timing`: 11.7 s to build, of which the fill is 96%.

Two things follow from the same formula. A smaller block is cheaper per voxel — `BLOCK`
4→2 alone takes the build to 7.9 s — and no block size removes the wall from the cost.

## Decision

Three pieces, none of which asks the hierarchy per voxel.

**Scatter.** `scatter.rs` buckets every face into the tiles its bounds grown by
`SEED_VOXELS` reach, then fills each tile from its own faces. A voxel is seeded only where
that list is complete, so the seed is exact, and the work follows the surface rather than
the volume around it.

**Carry.** `sweep.rs` propagates the *nearest face* — not the distance, not the side —
outward from the seed to the isosurface, eight directions over a tile and its one-voxel
halo, which is Zhao's sweep carrying a closest primitive. It runs on a lattice `CARRY = 2`
times coarser than the stored one, so crossing the wall costs an eighth of what crossing
it on the stored lattice would. Tiles are taken in the rings the walk of ADR 0069 found
them in, then swept once more over the whole shell: a tile only ever sees the faces already
in its own block, so a face can be stranded in a tile of the same ring that had not been
swept yet, and one further pass is what lets it across.

**Refine.** Each stored tile takes, per voxel, the nearest of the faces the eight carrier
points around it hold; the `CARRY³` voxels of one carrier cell share those eight, so they
are named once per cell. Where the carrier cannot be believed — nearer the mesh than
`TRUST_VOXELS` carrier steps, *and* near enough the isosurface that the value is not
clamped anyway — the hierarchy is asked for that voxel instead.

**The side is never carried.** Both sides of a face are upstream of some voxel and the
sweep cannot tell them apart; carrying it inverted 228 voxels on a ball of radius 10 and
broke the surface into 229 shells. It is worked out from the face where the values are
written, once per 4³ block wherever no lattice edge of that block is crossed.

## Consequences

On the 818k-face ball at 0.2 mm with a 2 mm wall: the build goes from 11.7 s to 5.3 s, and
a whole hollow at precision 0.6 from 13.2 s to 8.2 s. Signed by winding number, 12.1 s to
7.3 s. Wall thickness is no longer a multiplier on the fill.

It is not the order of magnitude step 10a aimed at, and the shape of what is left is
known: the carrier is now the larger half, two passes of it over 25 000 tiles, and the
refine the rest. The signal to reopen this is a carrier that can be crossed in one pass —
which needs an ordering finer than the walk's rings, since a carrier tile spans wider than
a 2 mm wall.

The approximation is real and is now pinned by a test:
`the_sweep_answers_what_the_hierarchy_would` compares the built field against
`Bvh::closest` over the whole band, at `iso = 0` and at `iso = -2`, and requires half a
voxel. `CARRY = 4` fails it; `CARRY = 2` and `3` pass, and 2 is kept because it also has
to divide `TILE`. Four of the eight sweep directions fail it at 0.69 mm. Any of those
constants moving needs that test rerun, not an argument.

## Alternatives considered

### Shrink the gather's block and keep it

`BLOCK` 4→2 is one constant and takes the build to 7.9 s and a hollow to 10.7 s, because
the gathered list is linear in the block. Rejected as the whole answer: it leaves the wall
thickness in the cost, and the ceiling is right there.

### Sweep the nearest face at the stored resolution

The first cut of this. It pays for the wall's volume at the band's resolution — 299 000
tiles swept against 92 000 stored — and came out *slower* than the gather on a real hollow,
12.9 s against 10.7 s. That measurement is what forced the coarse carrier.

### Propagate the closest point rather than the face

Danielsson's vector transform: distance becomes a subtraction, with no lookup into the
mesh at all. Rejected on memory — twelve bytes a voxel over a hundred-million-voxel shell
is a gigabyte, against four for a face index.

### The decision above, and what it costs

The field is no longer exact. It is exact in the seed, exact wherever the refine falls back
to the hierarchy, and an approximation everywhere else, bounded by a test rather than by
an argument. A model whose surface folds back on itself inside a wall thickness is where
that bound is weakest, and a ball is not that model.

It is also three moving parts where there was one, with four constants — `SEED_VOXELS`,
`CARRY`, `TRUST_VOXELS`, and the eight directions — that only a measurement justifies.
`TILE_COST_BYTES` of ADR 0063 was measured against the old fill and now prices a build
that allocates differently; it has not been re-measured.
