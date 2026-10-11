# 0223. Pin a cut by an added shell and a drain hole

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

A model cut in two to fit the volume is glued back, and two flat faces slide while the
glue sets. The Cut tool offers alignment pins (ADR 0218): a peg on one half, a socket on
the other. The workspace has no mesh boolean. It has two things that behave like one: the
slicer fills by the non-zero winding rule, so two overlapping closed shells print as their
union, and a drain hole is an inward-wound body appended at slicing time, which takes
itself out of the solid (ADR 0071, 0188).

## Decision

With pins on and both halves kept:

- `core_geometry::pin_spots` finds up to N points on the lower half's cap, each a radius
  and half a radius again from its edge, spread by farthest-point sampling over a 40-step
  grid, and `core_geometry::pin` builds a closed 32-sided cylinder.
- Each pin is appended to the lower half's mesh, sunk 0.1 mm into the cut, so the two
  shells overlap and print as one.
- Each socket is a drain hole on the upper half (`ModelHollow::add_drain`), 0.2 mm wider
  each side and 0.2 mm deeper than its pin. The project, undo and the slicer already carry
  drain holes, so nothing new is written down.
- With "lay the halves flat" on, the pinned half stays as it was cut: it cannot lie on its
  pins.

## Consequences

The pins and sockets need no new data in the project and slice exactly. A pinned half is
two shells, and a field built on it with the pseudonormal sign is off inside the 0.1 mm
overlap; hollowing such a half reads one wrong sliver at the surface. Sockets show in the
Drain tool and are cleared with the other holes there. A cap too small takes fewer pins, or
none, and the stage notice says so. Reopen this if a mesh boolean lands in
`core-geometry`: the pin should then be welded into the half.

## Alternatives considered

### Loose dowels, holes in both halves

Simpler geometry, but the user has to print or buy the dowel and align it by hand.

### A voxel union through `core-volume`

Exact union, but it remeshes the whole half on a lattice and loses its detail, as a relief
does, for a few millimetres of peg.

### Overlapping shells, and what it costs

A pinned half is not a single closed shell, which anything that assumes one has to bear.
