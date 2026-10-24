# 0328. Let only a tip meet the model: turn a head for room and cut a pad back

- **Status:** Accepted; superseded in part by 0331 (a head with no room under it) and 0341 (a head with no clear direction)
- **Date:** 2026-10-22

## Context

A support may touch the part only at its tip; anything thicker that meets the model cures
into it and leaves a scar where it is broken off. Two places let it. A head — the `[top]`
segment widening from the contact to the pillar — was asked about along its axis alone
(ADR 0249), and the top of the trunk under it is left out of the clearance sweep (ADR
0077), so under a curled finger or beside a lip the widening cone ran into the model. And
a pad the model stands beside is moved aside whole (ADR 0239); where it cannot move far
enough the support is dropped, which leaves its face unheld. And a pad was tested by rays
down its rim and axis, which pass by a toe standing inside it, so a part stood in the pads.

## Decision

**A head is swept whole.** `neck_of` sweeps the cone from contact to neck, with
`clearance_mm` round it, from where it stands clear of its own face's plane. One that
meets the model turns: straight down, then 15, 30 and 45 degrees off its direction in
eight ways, each still descending at most 45 degrees and leaving its face; the first clear
with its clearance wins, then the first clear bare. Where none is the head keeps its
direction, so no support is lost to this. The stretch of trunk just under a head, which
the clearance sweep skips, is swept bare.

**A pad is tested from its middle out.** Rays run across the plate from its middle in
48 directions at its sole, middle and top, and the pad is clear where none meets the model
within its radius and clearance. **A pad that cannot move is cut back.** When moving fails, `Landing::pad_cut` records how
far the pad reaches in each of 16 sectors: the nearest model those rays find along the
sector's edges and middle, less `clearance_mm`, capped at its radius. It is
taken only if every sector still covers the flare. The mesher pulls the foot's vertices in
to it, with 32 sides to a round pad so the cut keeps its shape. The cut is in the project
file; turning a tree turns it.

## Consequences

Heads no longer cure into the part beside their contact, and supports low beside a part
that ADR 0239 dropped now stand on a pad cut short on that side. A head in a crevice too
narrow for any direction still touches, as before; reopen if such scars are reported. A
turned head runs more beside its face, and each blocked head costs up to 25 more sweeps,
paid only where the first fails.

## Alternatives considered

### Drop a support whose head cannot clear

Strict, but it leaves exactly the faces that need holding unheld; the user asked for no
needed support to vanish.

### Shrink the pad evenly, as ADR 0237 did

Rejected by ADR 0239 for leaving a button that holds nothing; a sector cut keeps the pad
whole away from the model.

### The option that won, and what it costs

Two more rules in placement, a project-file field, and a cut measured by rays, which a
feature narrower than the gap between two of them can still poke into by less than the
clearance.
