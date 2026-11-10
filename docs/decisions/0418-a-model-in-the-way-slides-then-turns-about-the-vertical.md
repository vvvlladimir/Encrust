# 0418. Move a model onto the plate by a slide, then by the least turn about the vertical

- **Status:** Accepted
- **Date:** 2026-11-10

## Context

Check's In the way card offered Move clear and Arrange all. Both only slide a model across the
plate, so a model longer than the plate is deep, but short enough to fit across its diagonal,
stayed past the build volume. The only button that helped was Fit the volume, which scales the
model down. Two buttons for one issue also made the user choose between a small move and a
whole new layout before seeing either.

A turn about the vertical keeps every overhang as it was. Support points are kept in the
model's own space and regrow where the model now stands, so its supports keep their places on
it. A tilt changes which faces hang over the plate and needs supports chosen again.

## Decision

Arranging tries these moves in order, and stops at the first that fits:

1. A slide, if the model's box with its supports fits the usable area as it stands.
2. The least turn about the vertical, through the middle of that box, that fits the model's
   outline seen from above. `core_geometry::fitting_turn` finds it exactly over the convex
   hull with rotating calipers and takes the turn of smallest size. `core_plate::turned_onto`
   asks first with the plate's margin, then flush to its edge.

A model is never tilted to fit. Check's card has one button, Place on the plate
(`clear_the_way`): it slides only what is in the way, turns what no slide fits, and packs the
whole plate when no clear place is left for one. Arrange the plate turns the same models
before it packs. The command line's `--arrange` and the recipe's `arrange` step, with or
without `only_in_the_way`, do the same.

## Consequences

- A long model that fits across the plate no longer needs to be scaled down. Fit the volume is
  still offered while the model's box is larger than the volume, even where a turn would fit
  it.
- The least turn often leaves a model at a slant rather than a quarter turn. That is the
  smallest move, and packing is less tidy for it.
- The outline is read from every vertex of the model and its supports, only for a model no
  slide fits; the hull's sort grows with that count, n log n.
- Reopen this if users ask for a quarter turn ahead of a slant, or for a tilt before scaling.

## Alternatives considered

### Tilt onto a side, or any of the 24 square poses

Lying on a side fits more models, but it changes the overhangs, throws away the supports and
the user's choice of pose. Auto-orient already searches poses with the volume as a limit.

### Keep Move clear beside Arrange all

Both answer the same issue. One button that makes the least move, and moves everything only
when it has to, makes the choice for the user. Arrange the plate stays in the Models panel.

### The option that won, and what it costs

A model whose box is too tall, or too wide for any turn, is still left to Fit the volume or
Auto-orient. A slant placement can block a corner that a quarter turn would have left free.
