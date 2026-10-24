# 0323. Let presets differ in thickness only, place densely, and let a tip meet the surface

- **Status:** Accepted; the preset numbers and the one overhang angle are superseded by 0355
- **Date:** 2026-10-22

## Context

The shipped Light, Medium and Heavy of ADR 0254 placed contacts 4 mm apart scaled by a
per-preset density, held each preset to its own overhang angle, and sank every tip 0.2 mm
into the part. So a preset changed where supports went as well as what they looked like,
and the lattice was sparse: a flat underside got one contact per 16 mm² or less. Placement
is about to choose a support's shape by what each contact holds (an island by its area, an
overhang by the volume above it), which only works if where contacts go does not depend on
the preset that builds them.

## Decision

**A preset is a shape.** Contact, cone length, pillar and pad are 0.3/2/1/6×0.25,
0.6/3/1.3/9×0.5 and 1/4/3/12×0.5 mm for Light, Medium and Heavy. The flare is as wide as
the pillar, so there is none. A trunk grows at most 30 % past its pillar (1.3, 1.7,
3.9 mm), merges within 10 mm at 45 degrees, and a small pillar runs at most 5 mm.

**Placement is the same for all three.** Contacts are 1.8 mm apart at a density of one,
surfaces leaning past 40 degrees from vertical are held, every part is lifted 5 mm, bodies
keep 0.5 mm of clearance, and every preset may stand on the part. A heavier preset is
thicker, not denser and not stricter; ADR 0254's per-preset angle and densities are
withdrawn.

**A tip may meet the surface without biting into it.** `contact_depth_mm` and the small
pillar's depths may be zero, and the presets ship zero: the layer that prints the surface
cures into the contact under it, and the scar is the contact's width and no deeper. ADR
0243's mechanism is unchanged for any bite above zero.

**A tip without a bite sits on the surface.** Automatic placement puts a contact a layer
under the surface it holds, which a biting tip sinks back through. A tip without a bite is
moved up onto the face over it when its column is built, if that face looks down and is no
more than 0.5 mm above; what placement stores is unchanged, so a second run still reads it
as before.

## Consequences

- Contacts are several times denser than before: more supports and more resin on every
  part, finer and more even marks.
- Heavy now branches and may land on the part; its 3 mm pillar needs room a 1 mm feature
  does not give, so tight undercuts fall to its small pillar or are not built.
- A biteless contact under a face more than 0.5 mm away is taken to be off the surface on
  purpose and stays where it is.
- The spacing is one number until placement learns spacing by surface angle.

## Alternatives considered

### Keep per-preset density and angle

What ADR 0254 shipped. Rejected because shape by load needs one placement for every shape:
a heavier shape chosen for a heavy region must not also move its contacts.

### Keep the 0.2 mm bite

ADR 0243 found an unbitten disc falling away. Rejected for the shipped presets because a
bite deepens every scar and the denser lattice spreads the load; the field still takes any
bite a user wants.

### The option that won, and what it costs

A biteless contact holds by one layer curing into it and nothing more, so it is the first
thing to look at when a support falls away on a print.
