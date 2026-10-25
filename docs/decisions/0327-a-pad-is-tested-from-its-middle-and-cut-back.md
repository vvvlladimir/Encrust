# 0327. Test a pad from its middle out, and cut it back where it cannot move

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A pad the model stands beside is moved aside whole (ADR 0239); where it cannot move far
enough the support is dropped, which leaves its face unheld. A pad was tested by rays down
its rim and axis, which pass by a toe standing inside it, so a part stood in the pads. And
every look a pad made round itself ran the whole way through the model whatever it hit: on a
figurine of a million faces most of the rays growing a forest took were a pad's.

## Decision

**A pad is tested from its middle out.** Rays run across the plate from its middle in
48 directions at its sole, middle and top, and the pad is clear where none meets the model
within its radius and clearance.

**A pad that cannot move is cut back.** When moving fails, `Landing::pad_cut` records how
far the pad reaches in each of 16 sectors: the nearest model those rays find along the
sector's edges and middle, less `clearance_mm`, capped at its radius. It is taken only if
every sector still covers the flare. The mesher pulls the foot's vertices in to it, with
32 sides to a round pad so the cut keeps its shape. The cut is in the project file; turning
a tree turns it.

**A pad looks only as far as it reaches.** Each look stops at the pad's radius plus its
clearance (`Bvh::raycast_within`). Whether a height sets off inside the model is asked once
per height along a ray of any length, so a pad deep inside a wide part still has no room.

## Consequences

Supports low beside a part that ADR 0239 dropped now stand on a pad cut short on that side,
and no toe of the part stands in a pad. On a ball standing on the plate `grow` took 70 ms
rather than 102 ms (`benches/branch.rs`) once looks were bounded; on a model lifted clear of
its pads nothing changes.

## Alternatives considered

### Shrink the pad evenly, as ADR 0237 did

Rejected by ADR 0239 for leaving a button that holds nothing; a sector cut keeps the pad
whole away from the model.

### Drop a support whose pad cannot move

Strict, but it leaves exactly the faces that need holding unheld.

### The option that won, and what it costs

A project-file field, a cut measured by rays, which a feature narrower than the gap between
two of them can still poke into by less than the clearance, and three unbounded rays per pad
where the first of its looks used to answer.
