# 0331. Keep short supports between two faces of the part small pillars, laid on both faces

- **Status:** Accepted
- **Date:** 2026-10-23

## Context

Three rules turned a support between two faces of the part into something worse than the
small pillar it should have been. A head that could turn nowhere kept its direction (ADR
0328); over a thin bar of the model nearer than a head is long it ran through the bar and
stood its trunk underneath. Tips close together merged into one full-width support
standing on the part, a joint between two faces that has to be cut away. And every small
pillar with a neck was run into a support in its reach (ADR 0251), so one a millimetre
over the part became a thin strut leaning 7 mm across to a trunk. Its ends were discs
square to its axis, which on a sloping face meet it only at one edge.

## Decision

- **No head through the model.** A contact whose head finds no clear way and has the
  model nearer under it than a head is long has no head: `grow` runs it as one strut from
  the contact straight onto what is under it.
- **Split rather than merge on the part.** A merged front that stands on the model is
  split back into one tree per tip where every tip stands on the model as a small pillar
  of its own, from its neck or straight from its contact.
- **Into a support only when nearer.** A small pillar is run into a support beside it
  (ADR 0251) only where the strut there is no longer than its own drop to the part.
- **Both ends laid on their faces.** A small pillar's upper ring is laid on the face it
  holds and its lower ring on the face it stands on, as a plane tip's disc is (ADR 0330).

## Consequences

Short supports between two faces of the part come off as thin struts touching each face
with their whole end, and none crosses the model. More of them stand on the part than on
a support, so the part is marked at both ends of each; reopen if those lower marks are
reported. A small pillar whose head cannot clear but has open air below still keeps the
head of ADR 0328.

## Alternatives considered

### Drop a support whose head would cross the model

It leaves the face unheld, which ADR 0328 already rejected.

### Keep merging, and build the merged trunk on the part thin

A trunk carrying several tips is what a thin strut cannot hold, which is why ADR 0042
keeps the small pillar to one tip.

### The option that won, and what it costs

Each merged front standing on the part is grown again per tip, which costs placement time
only where it happens, and a split forest has more struts to brace and to cut away.
