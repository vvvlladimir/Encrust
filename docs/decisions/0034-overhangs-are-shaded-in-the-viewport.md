# 0034. Shade overhangs in the viewport shader, by face angle

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

The Supports tool asks the user for an angle — `max_overhang_deg` — and then, on Generate,
fills the plate with columns. Between those two moments there was nothing to look at. A
user who wanted to know what the number meant, or which parts of a model were about to be
covered in supports, had to run the placement and read the result.

Every other slicer answers this by tinting the model where it hangs over: the faces that
lean past the threshold are painted live, as the slider moves.

What placement itself measures is not a face angle. It measures area that appeared with
nothing cured under it, on the slice stack (`docs/decisions/0029`, `docs/decisions/0031`).
Showing exactly that would mean cutting the plate on every change of the angle — hundreds
of milliseconds of work on a worker thread, for a slider that moves continuously.

## Decision

The viewport washes a face with the overhang colour when its outward normal points further
down than the profile's angle allows, and it does so in the fragment shader.

Each model instance carries one number, the sine of the angle a surface may lean from
vertical: a wall is zero and a ceiling is one, which is exactly the downward part of a unit
normal. The shader compares that against `-normal.z` and mixes the colour in over a narrow
band, so a curved surface shades into the mark instead of breaking along a ring of
triangles. A negative number means "do not mark this mesh", which is what support columns
are drawn with — scaffolding does not need holding up.

The mark is shown while the Supports tool is the tool in hand, and follows the tool's own
profile, so moving the angle repaints at once with no work outside the frame.

The shading uses the face's outward normal, not the one turned to face the camera, because
which way a surface hangs does not depend on where the camera is.

## Consequences

- The angle is now a number with a picture attached: the model repaints as the slider moves
  and nothing is computed outside the GPU.
- It costs nothing measurable. One float per instance and three instructions per fragment.
- **What is shown is not exactly what will be supported, and both directions are visible.**
  A face that leans past the angle but rests on the plate, or on the model below it, is
  marked although it needs nothing; a flat island one layer thick is marked, and is held,
  but a thin peninsula whose faces are all vertical is not marked and *is* held. The mark
  is an indication of steepness, and placement remains the answer.
- Mesh-space normals are used, so a model scaled non-uniformly is marked by its shown
  surface rather than by its original one — which is right, because the shown surface is
  what gets printed.
- The signal to reopen this: a user reading the mark as a promise. The honest fix is to
  paint the contacts a run would place, once a placement result can be kept and drawn
  without re-cutting the plate, which is a step 8 question.

## Alternatives considered

### Show the real answer: the unsupported areas from the slice stack

The only version that cannot mislead. It needs the plate cut at the print's own layer
height before anything can be drawn, on every change of the angle, and the result is a set
of areas on layers rather than a surface — drawing it means either extruding those areas
into geometry or shading against a texture stack. Both are a step of their own.

### Mark on the CPU, per face, and colour the vertices

The same test, done once per import and stored on the mesh. It avoids nothing — the test is
three instructions — and it has to be redone when the angle changes, when the model is
turned, and when it is scaled, each of which means re-uploading every vertex.

### A separate pass drawing only the overhanging faces

A second draw with its own pipeline over a filtered index buffer. More code, another
pipeline to keep in step with the first, and the filtering is the CPU-side version above.

### The option that won, and what it costs

Face angle is the wrong measurement for MSLA, and this project has an ADR saying so. The
mark will show red where nothing will be supported — the underside of a part sitting flat
on the plate is the common case — and will show nothing on a flat island that is the most
dangerous thing on the plate. It is shown anyway, because it answers the question the
slider raises the moment it is touched, and because every slicer a user has met shows the
same thing in the same colour. The Supports tool still has to be the one in hand for it to
appear, which keeps it out of the way of judging a model's shape.
