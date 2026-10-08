# 0208. Carrying a support by hand is settled in core-supports

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

ADR 0095 decided what a carried support may become: every picked part takes the same step,
a tip is pulled back onto the nearest surface it holds, and a step that sinks any body of
the tree into the model or lands a tip on a blocked face is dropped. `core-supports`
already owned half of that rule — `fits` sweeps the beams and `on_model` finds the surface
a tip belongs on — but the other half lived in the viewport panel that draws the drag:
which nodes a `Part` moves, that a foot stays on the plate, and the order the snap and the
sweep run in. Four private functions of geometry, a hundred lines, in the file that also
steers the camera and paints overlays.

Nothing but a window could reach them, so nothing but a window could test them: the rule
that decides whether a hand-made support is printable had no test of its own, while the
automatic placement it mirrors has many.

## Decision

`core_supports::carried` takes a tree, the parts held, the step, the model it stands
against and its profile, and answers with the carried tree or `None` when the model
refuses the step. Deciding which nodes a part moves, keeping the foot on the plate,
snapping the tips and sweeping the result are its business, in that order.

The window keeps what is a window's: which parts are held, where the cursor's step comes
from, freezing on the first touch, and putting the answer back into the scene.

## Consequences

The editing rule is tested where the placement rule is tested, against bodies with a
closed-form answer — a foot carried up still stands at `z = 0`, a tip carried off a shelf
comes back to its underside, a foot dragged under a cube is refused — and it is reachable
from a benchmark or the command line if hand-editing ever arrives there.

`core-supports` gains one public function and the `Part` enum it already exported becomes
an input and not only an answer from `grab`. The viewport panel loses a hundred lines of
geometry and keeps only the drag.

What has to be watched is the boundary: a front end that wants a part of the rule without
the rest — carrying with no snap, say, for a drag that is still in flight — would have to
split `carried` rather than copy its insides back out. The signal to reopen is a second
caller needing the steps apart.

## Alternatives considered

### Leave it in the panel and add a test harness for the window

The functions are small and the panel is where the drag is read. Rejected: testing them
needs a `Window`, a scene and a camera, so the test would be about the window and not
about the geometry, and `fits` and `on_model` were already next door in `core-supports`
with tests of their own.

### A method on `SupportTree`

`tree.carried(parts, step, placed, profile)`. Rejected: `SupportTree` is data — nodes, a
landing and a group — and the rule is an algorithm over the model beside it, which is the
split the architecture rules draw (rule 4). `fits` and `on_model` are free functions for
the same reason.

### The option that won, and what it costs

One function with five arguments, which is wide, and it hides an order that matters: carry,
then snap, then sweep. A caller that wanted another order has no way to ask for it without
the function being split. That is the price of the rule having one place to live, and the
order is the one ADR 0095 fixed.
