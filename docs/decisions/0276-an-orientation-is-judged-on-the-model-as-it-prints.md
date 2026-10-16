# 0276. Judge an orientation on the model as it prints: sized, hollowed and drilled

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

`orient` took one mesh, and the window handed it the model's file as drawn. Three things
the user had already done to the model were not in it:

- a scale or a stretch. The 30 N limit, the 25 mm² cup and the contact spacing are plate
  millimetres, and a pull grows with the fourth power of the size; a model squashed along
  one axis was searched as the shape it no longer has;
- a cavity. A hollowed model cures rings, which pull a fraction of the solid section, and
  its cavity is a suction cup whichever way it stands until something lets the air in;
- drain holes and channels. Where they stand decides whether that cavity vents from the
  first layer or only once the hole is reached.

A cut and a pressed relief replace the model's mesh, so they were already seen.

## Decision

- `core-plate` gains `Printed`: the outer surface, the shell where there is one, the cut
  bodies wound inward, and the way out of each hole. `orient_printed` takes it, and
  `orient` is the same for a solid model.
- Supports are judged on the outer surface alone. Pull and cups are read off sections of
  the shell with the cuts appended, by the winding rule the stack is filled by.
- A shell is cut every millimetre, up to 128 sections, since a cavity's floor is one wall
  thick and seals nothing in a section that misses it. Every run of cuts gets planes of
  its own half a millimetre apart, up to 16, so a hole is met inside the wall it crosses.
- Each hole pointed at the plate is a candidate and is always cut, up to eight of them:
  the sphere's samples stand 14° apart, and a hole vents a cavity's floor only within a
  few degrees of straight down.
- The window bakes each mesh through `Transform::sizing` before the search, on the worker
  thread, and only for a model that was resized.

## Consequences

A hollowed model with a hole is stood on that hole where the tilt allows, and a hollowed
model with none is ranked among orientations that all cup. A resized model is searched as
the shape and size it prints at. A hollowed model costs up to five times the sections of
a solid one, and a resized one a copy of its mesh for the length of the run.

The command line still orients before it hollows, so `--orient` with `--hollow` judges the
solid. Reopen this if that order is changed, or if a cavity's own ceiling turns out to
need supports the outer surface does not account for.

## Alternatives considered

### Hand `orient` the baked mesh the stack is cut from

One mesh and no new type. But the cavity's faces look down under their own ceiling and
would be charged as overhang on a surface no support stands under, and the cut bodies,
laid eight deep, would count eight times.

### Find the venting orientations by sampling the sphere more finely

No hole directions to pass. Reaching a few degrees takes thousands of samples, each one
cut, for a direction the model already carries.

### What the chosen option costs

The cup rule is still read at sections, not layers: a cavity floor thinner than a
millimetre, or a hole meeting its wall between two planes, is missed. Supports inside a
cavity are not counted at all.
