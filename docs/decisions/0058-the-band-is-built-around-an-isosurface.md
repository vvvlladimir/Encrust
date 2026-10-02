# 0058. The band is built around an isosurface, not around the mesh

- **Status:** Accepted
- **Date:** 2026-09-21

## Context

Hollowing needs the surface `d = -t`, the model's own surface pushed inward by the wall
thickness. Step 8c already has an operator for that: `shell(t)` subtracts the wall from
every value of a field built around the mesh.

It cannot be used. A field carries distances only inside its band, and refuses to move a
surface further than the band reaches (ADR 0057). A wall of 2 mm at a 0.2 mm lattice is a
band of ten voxels rather than three, and a tile is kept wherever the band crosses, so the
stored volume grows with the band: the same 40 mm ball costs 3 300 tiles at a three-voxel
band and roughly 30 000 at the band a 2 mm wall would need. A 4 mm wall doubles that
again. The narrow band stops being narrow exactly when the operator is worth having.

Nothing about that is necessary. The distance to the inward offset of a mesh is
`d(p) + t`, and `d(p)` comes from `Bvh::closest` at whatever point is asked, at the same
cost wherever the point is. The band does not have to be centred on the mesh to be narrow;
it has to be centred on the surface being extracted.

## Decision

`FieldSettings` carries `iso_mm`. A field stores `d(p) - iso_mm` clamped to the band, and
keeps the tiles that surface crosses. `iso_mm = 0` is the mesh, a negative value its
inward offset, a positive one its outward offset.

Three things follow from the band no longer sitting on the mesh:

- **Candidate tiles** are the tiles a face reaches across `band + |iso|`, rather than
  across the band.
- **The tile reject** is on the *signed* distance at the tile's centre, not the unsigned
  one. `||s| - |iso||` is also a valid bound, but it cannot tell the inward offset from
  the outward one, and would fill the shell on the wrong side of the mesh as well — twice
  the work for nothing. On the 40 mm ball that is the difference between 1.8 s and 1.1 s.
- **The row skip** is bounded by how far the *value* is outside the band, rather than by
  the distance to the mesh. A signed distance moves by at most the distance walked, so a
  voxel whose value is `v` stays outside the band for `|v| - band` millimetres whichever
  way the row goes. With `iso = 0` the two bounds are the same number; with the band
  around an offset they are not, and the difference is most of the wall.

`shell` and `offset` stay as they are. They are still the right answer for moving a
surface a fraction of the band.

## Consequences

Hollowing a 2 mm wall costs what hollowing a 0.2 mm wall costs: the wall thickness moves
where the band sits, not how big it is. A 60 mm ball hollows in 1.5 s at the default
precision, and a bottom-through hollow — which builds a second field at `iso = 0` to clip
its prism — in about a second more.

The field's *distances* away from its own band are no longer the distances to the mesh:
they are the distances to the isosurface, which is what every consumer of a hollowing
field wants. A caller that wants the mesh's own field asks for `iso_mm = 0`, which is the
default, so nothing built before this changed.

Two fields of different `iso_mm` still share a lattice and still combine, because a field
carries no memory of what it was an offset of. That is what lets `open_bottom` take the
`max` of a cavity banded at `-t` and a solid banded at `0`.

The signal to reopen this: an operator that needs distances over a wide region rather than
a surface — a medial axis, a real diffusion — would want a wide band and would not be
served by moving a narrow one.

## Alternatives considered

### Widen the band and use `shell`

One operator, no new field on `FieldSettings`, and exactly the code step 8c shipped. It
loses on memory and time together: the tile count grows with the band, and every one of
those tiles is filled by nearest-point queries that the result then throws away, because
only the tiles around `d = -t` survive the shell. It is the right answer only for a wall
of about a voxel, which is not a wall.

### Build the mesh's field once and resample it for each wall

Keeps one field per model and makes changing the wall cheap. It needs the distance far
from the surface to be stored to be resampled, which is the dense field ADR 0057 ruled
out; a narrow band has nothing to resample.

### The option that won, and what it costs

`iso_mm` is a third thing a caller has to understand about a field, and it makes
`FieldSettings` describe a surface rather than a mesh. The tile reject now costs one
sign query per candidate tile — a `Signer` call the previous test did not make — which is
paid on every build, including the `iso = 0` ones that do not need it; on a fine lattice
that is under a percent of the build, so it is not worth two code paths.

More honestly: a field built at `iso ≠ 0` is a perfectly good `Sdf` that quietly means
something different from its siblings. Nothing in the type says so. A caller that builds
a field at `iso = -2` and then calls `offset(field, -2)` on it gets a surface 4 mm inside
the mesh and no complaint. The mitigation is that `hollow` is the only caller that passes
a non-zero `iso_mm`, and it is in the same crate.
