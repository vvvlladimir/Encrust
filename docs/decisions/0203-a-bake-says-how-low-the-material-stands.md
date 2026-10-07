# 0203. A bake says how low the material stands, and the plate is warned about that

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

ADR 0199 took the cut bodies out of the top of the stack: `Baked` carries `ceiling_mm`, the
top of the material alone, and nothing is planned above it. The bottom was left where it was.
`core_slicer::on_the_plate` clamps the stack to the plate and warned, off the merged mesh's
own box, that "the model reaches under the plate". A merged box holds the cut bodies, and a
drain hole drilled into the underside of a model reaches below it by design — so a 30 mm
sphere with a hole in its lowest point warned that a tenth of a millimetre of it was being
lost, when nothing of it was. The warning fired on exactly the models whose drainage is
right.

The slicer cannot tell the two apart: it is handed one mesh. The caller that merged it can,
and already does for the ceiling.

## Decision

`Baked` carries `floor_mm` beside `ceiling_mm`, by the same rule: the bottom of the models
and their supports, with the bodies that only subtract left out. `core_engine::cut` is where
the under-plate warning is raised, from that floor. `on_the_plate` still clamps the stack to
the plate and says nothing.

## Consequences

A hole drilled into the underside of a model is silent, and a model that has really been
pushed under the plate — by a relief pressed into its bottom, by a transform typed by hand —
still says so once per run. Every front end gets this from `cut`, so the window and the
command line agree. `Baked` built by hand needs the field filled; `Baked::of`, for a mesh
that carries nothing but material, fills both from its box.

core-engine takes `tracing`, which fourteen crates of the workspace already take. Should the
floor ever need to mean something else — a model deliberately printed into the plate — this
is the one place to revisit.

## Alternatives considered

### Pass the floor down to `on_the_plate`

The slicer would then clamp the stack to the material rather than to the merged box, which
would also stop the empty layers under a hole drilled into a floating part. It costs a fourth
parameter on `layer_heights_under`, `adaptive_plan_under` and `Windows::under`, all three of
which exist only because the ceiling had to travel the same way. The empty layers under a
floating part are a separate finding with its own measurements; when it is taken on, the span
— floor and ceiling together — is the shape to give these three.

### The option that won, and what it costs

The warning now lives one crate above the code it is about, so a reader of `on_the_plate`
cannot see that anybody says anything about the clamp it applies. The stack is still planned
from the merged box, so a hole under a floating part still plans the layers between the hole's
mouth and the part — blank masks, as before this decision.
