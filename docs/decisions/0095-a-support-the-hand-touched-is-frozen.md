# 0095. A support the hand has touched is frozen into the scene

- **Status:** Accepted
- **Date:** 2026-09-26

## Context

A support has parts a hand wants: the tip, the strut under it, the joints a branch meets
at, the trunk and the foot. Other slicers let each of them be dragged. Here a tree was a derived
value — points in, `columns` and `grow`, trees out — rebuilt whenever the placement, the
profile or the paint changed. Any edit to a joint would survive until the next rebuild and
no longer, which is a few milliseconds.

Editing therefore needs somewhere for hand-made geometry to live, and that somewhere has
to be the scene, because the scene is what the undo stack snapshots (ADR 0086).

## Decision

The first time a support is taken hold of it is frozen: the tree as it stands is copied
into the object, in the model's own space, and the points its tips grew from are taken out
of the automatic run. From then on nothing regrows it — a rebuild maps it into plate
coordinates and meshes it with the rest — and every part of it can be carried.

`grab` answers which part a ray met: a node, the strut under a node, the trunk or the
foot. Nodes and the foot are aimed at as balls a little wider than the sticks, and they
win a tie, because a joint is the smaller target and the one that was meant.

A press picks; the press after that carries. A part already picked is carried by the step
the cursor takes on the plane through it facing the camera, and every other picked part
takes the same step, so several parts of several supports move together. Shift adds to
what is picked. Carrying a node bends whatever meets there; carrying a strut moves both
its ends; the foot stays on the plate.

A carried support is held to the rule the automatic run places by. Before a step is kept,
every tip of the support is pulled back onto the nearest surface it holds — a support that
has stopped touching the model holds nothing — and the whole tree is swept as beams with
the profile's clearance, tip heads apart, exactly as `landing` does when a support is
first put down (ADR 0077). A step that would sink any part of it into the model, or put a
tip on a blocked face (ADR 0093), is dropped and the support stays where it was.

A frozen support is handed back with **Grow these again**, which turns its tips back into
points.

## Consequences

An edit cannot produce a support that would not have been placed there in the first
place, and it cannot leave one hanging in the air off the model. The cost is a beam test
per frame of a drag — a few dozen rays through the hierarchy, which is what one placement
already costs.

An edit holds. What it costs is that a frozen support stops taking part in everything
automatic: it never merges with a neighbour again, and it keeps the shape it had when it
was frozen even if its group's numbers change afterwards — only the thickness follows the
group. That is the trade the "grow again" button exists for.

A frozen tree is geometry in the scene, so it rides duplication and undo for free and it
is what the slicer sees. The undo fingerprint hashes every node of it, which is a few
dozen floats per support rather than three.

Freezing on the first touch means a stray click on a bed of automatic supports quietly
takes one out of the automatic run. Nothing says so beyond the panel's counts. The signal
to reopen is a user surprised that a support stopped merging after they looked at it.

## Alternatives considered

### Overrides on the point, as the hand-placed foot of step 13b was

A point with an optional base, an optional lean, an optional joint offset. Rejected: a
joint belongs to several points at once, so there is no point to hang it on, and the
overrides multiply with every part that becomes editable. The 13b base override is
withdrawn in favour of this.

### Storing the whole forest always, generated once

One kind of support instead of two. Rejected because it turns a profile change into a
regeneration: with the forest as the source of truth, nothing knows how to grow it again.

### The decision above, and what it costs

Two kinds of support live side by side — grown and frozen — and every piece of the window
that touches supports has to know which it is holding: removal, counting, seeding an
automatic run, and the numbering that shifts when a frozen one is taken away.
