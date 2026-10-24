# 0324. Space contacts by the angle of the face they hold

- **Status:** Accepted
- **Date:** 2026-10-22

## Context

Automatic placement put contacts one `contact_spacing_mm` apart under every overhang. A face
looking straight down and a face leaning just past `max_overhang_deg` got the same lattice,
though the first hangs on its supports alone and the second is mostly carried by the layers
under it. The 1.8 mm spacing of ADR 0323 is right for the first and several times too dense
for the second. Placement never reads a face's normal: an overhang is what a layer reaches
past the layer a millimetre under it (ADR 0031), so the angle has to come from the stack.

## Decision

**A profile has two spacings.** `contact_spacing_mm` is the spacing under a face looking
straight down, and `steep_contact_spacing_mm` (3.3 mm in the shipped presets) is the spacing
under a face leaning just past the overhang angle. Between them the spacing is linear in the
angle between the face's normal and straight down. A steep spacing under the other one is
taken as that one, and a file without the field reads 3.3.

**The angle is read off the stack.** A sampled overhang cell is put in one of four bands by
how far it lies past the layer a millimetre under it, the band edges being the leans a
quarter, a half and three quarters of the way from the angle to level; it takes the spacing
of its band's flattest face. A cell more than a step past the layer directly under it is a
shelf and level, whatever the bands say.

**The spacing is a ball, not a lattice.** The lattice is still laid at `contact_spacing_mm`;
each support placed holds a ball a quarter of that short of its own spacing, and a later
sample inside any support's ball is skipped. Under a level face the lattice spaces the
supports, as before; under a steep one each layer adds a sliver and the balls space them.

## Consequences

- A sloping underside takes roughly half the supports it did at the shipped numbers; a
  ceiling takes the same.
- A support keeps its own ball, so it is dropped from the run once the climb passes it, and
  the buckets are as wide as the widest ball.
- Orientation still scores a pose by `contact_spacing_mm` alone, so it counts supports a
  little high under sloping faces.
- Four bands make the spacing a staircase of four steps; reopen if the steps show on a print.

## Alternatives considered

### Reading the normal off the mesh

Exact per point, but placement works on the stack so that a contact belongs to the layer it
holds, and asking the mesh for every sample costs a ray or a nearest-face query each.

### A spacing per overhang piece

One `uncarried_by` per band per piece, and one spacing for a piece that is a ceiling on one
side and a slope on the other. Measured slower on a ball than asking the few sampled cells.

### The option that won, and what it costs

A second field in every profile, and a band that takes its flattest face's spacing, so a
face is held a little denser than the line between the two spacings says.
