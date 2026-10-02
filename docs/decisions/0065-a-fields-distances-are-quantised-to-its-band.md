# 0065. Store a field's distances as steps of its own band

- **Status:** Accepted
- **Date:** 2026-09-22

## Context

A tile held 512 distances as `f32`, two kilobytes a tile, and a field of a real model is
tens of thousands of tiles. Every one of those values is clamped to the band, which is two
voxels for hollowing — so the entire range a stored value can take is `[-band, band]`, and
`f32` spends its precision on exponents the band rules out.

## Decision

A tile holds `i16` steps of its own band: `i16::MAX` is the band, `-i16::MAX` is the band
on the inside. `Sdf` keeps the millimetres one step is worth, so reading a value is a
conversion and a multiply rather than a divide.

## Consequences

A tile costs one kilobyte rather than two, and a field half of what it did. On a band of
two voxels at 0.1 mm the step is six nanometres, which is four orders below the half-voxel
marching cubes is accurate to and below anything the lattice can express.

Nothing else changes: `value` and `sample` still answer in millimetres, and the operators
still work in them.

The reason to reopen this is a use of a field with a band far wider than a few voxels. The
step is the band over 32 767, so a band of 50 mm would still be quantised to a micron and a half — fine — but a
field that stopped clamping to a band altogether would lose the guarantee that makes this
safe.

## Alternatives considered

### `i8` steps

Four times smaller again. Rejected: a step is then the band over 127, which at a two-voxel
band is 1.6% of a voxel, and the crossing point marching cubes interpolates would land on
a visible stair.

### `f16`

Same size as `i16` and no scaling to carry. Rejected because it spends bits on an exponent
the clamp has already made pointless, giving fewer usable steps near the band than the
fixed point does, and because it needs a dependency or nightly.

### The decision above, and what it costs

The values in a tile are no longer distances. Every path that touches a tile has to encode
or decode, which is a rounding on the way in that a debugger will show as a value a
hairsbreadth off the one that was computed, and a test that compares a stored value to the
number it was built from needs a tolerance of one step rather than of `f32` error. Two of
them did.
