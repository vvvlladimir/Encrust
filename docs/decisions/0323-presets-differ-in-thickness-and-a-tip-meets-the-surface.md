# 0323. Ship presets that differ mostly in thickness, place densely, and let a tip meet the surface

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

The shipped Light, Medium and Heavy of ADR 0254 placed contacts 4 mm apart scaled by a
per-preset density, held each preset to its own overhang angle, and sank every tip 0.2 mm
into the part. So a preset changed where supports went as well as what they looked like,
and the lattice was sparse: a flat underside got one contact per 16 mm² or less. Sizing a
support by what it holds (ADR 0325) only works if where contacts go does not hang on the
shape that builds them. Prints of the round, checked supports of this work then set the
numbers: a 3 mm pillar on Heavy marked more than it held, thin pads lifted off the plate on
the peel, and bracing that started 3 to 8 mm up left the lower trunks free.

## Decision

**A preset is a shape.** In mm:

| | Contact | Head | Pillar | Trunk at most | Flare | Pad |
|---|---|---|---|---|---|---|
| Light | 0.3 | 2 | 1.0 | 1.3 | 2.0 | 6 |
| Medium | 0.6 | 3 | 1.3 | 1.7 | 3.0 | 9 |
| Heavy | 1.0 | 4 | 2.0 | 3.0 | 2.6 | 12 |

Every pad is 0.5 mm thick. Tips merge within 10 mm at 45 degrees on struts of at most 6 mm,
six tips to a trunk; a small pillar runs at most 5 mm; a pillar has 12 sides, as a file
without `facets` reads.

**Placement is shared but for one angle.** Contacts are 1.8 mm apart at a density of one,
every part is lifted 5 mm, bodies keep 0.5 mm of clearance, and every preset may stand on
the part. Light and Medium hold surfaces leaning past 40 degrees from vertical, Heavy past
45: a heavier preset is thicker and holds a little less, never denser.

**Bracing** is 0.9 mm across, at most 10 mm apart, from 1 mm up, zigzag at the bottom and in
the middle and crossed at the top (ADR 0328), rising every 5 mm, Light's every 10. Medium
props a free top with buttresses (pairs above 30 mm, one above 60); Light and Heavy do not.

**A tip may meet the surface without biting into it.** `contact_depth_mm` and the small
pillar's depths may be zero, and the presets ship zero: the layer that prints the surface
cures into the tip under it (ADR 0339). ADR 0243's mechanism is unchanged for any bite
above zero. Automatic placement puts a contact a layer under the surface it holds, which a
biting tip sinks back through; a tip without a bite, outside the part, is moved up onto the
face over it when its column is built, if that face looks down and is no more than 0.5 mm
above. What placement stores is unchanged, so a second run still reads it as before.

## Consequences

- Contacts are several times denser than under ADR 0254: more supports and more resin on
  every part, finer and more even marks.
- Heavy branches and may land on the part; its 2 mm pillar needs room a 1 mm feature does
  not give, so tight undercuts fall to its small pillar or are not built.
- Switching a model between presets can change which surfaces are held, Heavy's 45 degrees
  against 40; a test or benchmark measured against older numbers measures other supports.
- A biteless contact holds by one layer curing into it, so it is the first thing to look at
  when a support falls away on a print. Reopen if Heavy at 45 degrees leaves sagging
  overhangs.

## Alternatives considered

### Keep per-preset density and angle

What ADR 0254 shipped. Rejected because sizing by load needs one placement for every
shape: a heavier shape chosen for a heavy region must not also move its contacts.

### One overhang angle for all three

Shipped first in this work and kept sizing simplest, but the prints that set the numbers
above moved Heavy to 45 degrees.

### Keep the 0.2 mm bite

ADR 0243 found an unbitten disc falling away. Rejected for the shipped presets because a
bite deepens every scar and the denser lattice spreads the load; the field still takes any
bite a user wants.

### The option that won, and what it costs

Three presets that no longer share one placement exactly, and a lattice dense enough that
every part carries more resin in supports than before.
