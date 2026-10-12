# 0237. Fit a pad to the room it has, and lift a part by what its support needs

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

`bottom.platform_diameter_mm` was the width of every pad (ADR 0131: 7 mm Light, 12 mm
Medium), while contacts stand `contact_spacing_mm` apart, 4 to 5 mm. Under any dense run the
pads overlapped into one slab. Measured on `peel-slab` laid flat with Medium: 937 supports,
and the pad layer pulled 261 N on the film against the model's own 202 N and a limit of
30 N. Under a part whose supports stand in a ring the same slab is a closed cup on layer 0.
And a pad the model stood in the way of refused the landing outright, so a ball on the
plate kept 2 of 26 contacts.

Separately, `z_lift_mm` was 2 mm on every preset, and Medium needs 3.7 mm under its lowest
contact: a 1 mm pad, a 0.5 mm flare, a 2 mm top segment and the shortest pillar. Every low
contact got a neck cut short, and no tree had room to merge.

## Decision

**`platform_diameter_mm` is the widest a pad may be, not the width it is.** `foot_radii`
gives each support standing on the plate a pad narrowed to leave `FOOT_GAP_MM` (0.5 mm) of
air to the nearest other pad, tried at the whole, three quarters and half of that against
the model, and never narrower than the flare standing in it. A landing asks only whether
that narrowest foot clears the model.

**A shipped preset lifts a part by at least its own pad, flare, top segment and shortest
pillar.** Medium and Heavy go to 5 mm; Light already met it at 2 mm.

## Consequences

- No pad layer is the hardest pull of any test model any more; on `peel-slab` the peak is
  the model's own first layer.
- A lone support keeps the whole pad, so adhesion of a sparse run is unchanged; a dense
  run stands on many small pads, about 60% of the plate under it.
- Branching no longer always saves resin: under a low island the pads decide it, and a
  merged trunk's pad is the widest there is.
- A part printed with Medium or Heavy is 3 mm taller, 60 layers at 0.05 mm.
- Reopen if a dense run comes off the plate: the answer then is a shared, slotted raft.

## Alternatives considered

### One slotted raft under a dense run

Holds harder than separate pads and vents through its slots. It is a second foot shape, a
footprint to compute and clip against the model, and a profile section; nothing measured
says the small pads let go.

### Shorten Medium's top segment to fit 2 mm

Keeps the print short, and makes Medium a second Light: the segment's length is what lets
a tip reach in under a ledge.

### Fitting the pad, and what it costs

The width in the profile is no longer the width on the plate, so the panel's number is a
ceiling. Two pads closer than their flares still touch, which only a merge can prevent.
