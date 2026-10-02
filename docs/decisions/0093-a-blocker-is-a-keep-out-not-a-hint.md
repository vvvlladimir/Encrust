# 0093. A support blocker is a keep-out, not a hint

- **Status:** Accepted
- **Date:** 2026-09-25

## Context

The FDM slicers paint support blockers, and their users keep reporting the same thing: a
blocker only says "generate no support for this surface". A trunk grown from somewhere
else still lands on the painted area, because the blocker is a mark on the surface rather
than a rule about what may touch it. Most resin slicers have no blocker at all.

This workspace already asks, for every support body, whether the model is in its way
(ADR 0077), and every landing already comes back from a raycast that names the face it
met. A blocked face is therefore one test away from being a real keep-out.

## Decision

A blocked patch is a `Region` like any other (ADR 0092), and it forbids three things:
automatic placement puts no contact within a tip's radius of it, a fill skips its faces,
and `landing` refuses any foot whose raycast lands on one of them. A tip over a blocked
surface is not dropped: it leans or is carried by a branch exactly as a tip with no room
under it already is (ADR 0078), so a support pushed off a painted face steps past it
instead of disappearing.

To carry that answer everywhere it is asked, the model a support lives with travels as one
value: `Placed` — the mesh, its hierarchy, its placement and the blocked patch. It
replaces the three arguments every placement function already took together.

## Consequences

A painted face is honoured by every path that puts geometry against the model: a click, a
fill, an automatic run, a branch looking for somewhere to land. That is more than either
reference slicer does, and it is what makes a blocker usable on a face that matters — a
portrait's cheek, a mating surface — rather than a hint that the next run ignores.

The cost is one question per landing and one nearest-point query per automatic contact,
both against a hierarchy built over the blocked faces alone.

The keep-out stops at the surface: a beam may still pass a hair above a blocked face
without touching it, because what is forbidden is landing on the face, not entering the
air over it. The signal to reopen is a user asking for a volume they can keep supports out
of entirely.

## Alternatives considered

### The painted-blocker semantics — a blocker only suppresses generation

Half the work and what users already know. Rejected because it is the thing they complain
about: the blocker does not hold once branching moves a support.

### A blocked volume rather than a blocked surface

A box or a ball supports keep out of, tested against every beam. Rejected for now: it is a
second kind of marker to place and edit, and the surface answer covers the case people
actually paint — "not on this face".

### The decision above, and what it costs

Every placement function changed shape to carry `Placed`, which is a wide diff over
`core-supports` for a feature that only needed one more question answered. And because a
blocked tip leans rather than vanishing, a heavily painted model can end up with supports
reaching in from further away than the user expected; nothing warns them.
