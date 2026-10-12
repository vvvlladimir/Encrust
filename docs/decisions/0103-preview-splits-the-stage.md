# 0103. Preview splits the stage between the model and the mask

- **Status:** Accepted, hiding the columns superseded by 0217
- **Date:** 2026-09-26

## Context

Step 6 put the exposure mask in the inspector, under the printer and the resin. The
inspector is 300 points wide, so a 8520 x 4320 panel was drawn about 276 points across:
small enough that an island the size of a support tip is a pixel, which is exactly what a
layer preview exists to show.

Preview is the reading half of the application. Nothing in it edits the plate, so the tool
rail has nothing to offer it and neither has the plate panel.

Other slicers give the mask half the window in the equivalent mode, and put the layer
slider against that half rather than against the model.

## Decision

Entering Preview splits the stage in two: the model on the left, the mask on the right,
each taking half of whatever the stage has. The tool rail and the plate panel are not drawn
at all in that mode, so both halves are wider than the whole stage was.

The section rail and the mode switch are anchored to the stage rather than to the viewport,
so Preview parks the scrubber against the mask's edge and the switch stays over the middle
of the window.

The inspector then carries the layer's own numbers — its index, its Z, its exposure, and
the motion the machine runs around it, which the bottom block runs differently.

## Consequences

- The mask is drawn several times larger, and a thin feature is legible in it.
- `panels/inspector/mask.rs` split: the picture became `panels/mask_pane.rs` on the stage,
  the figures became `panels/inspector/layer.rs`.
- Half a stage is a fixed share rather than a draggable splitter, so a user who wants only
  the mask, or only the model, cannot have it. A splitter is the obvious extension if
  anyone asks.
- Leaving Preview brings the rail and the plate panel back, which is a visible jump in the
  layout. It is the same jump the mode switch already announces.

## Alternatives considered

### Keep the mask in the inspector and make the inspector wider

One number to change. Rejected because the inspector is sized for fields and labels, and
widening it for one picture in one mode would waste that width in every other mode.

### Float the mask over the viewport as a card

Consistent with the view tools and the section rail. Rejected because a card either covers
the model or is too small to be worth having, which is the problem being solved.

### The option that won, and what it costs

A fixed half is arbitrary: on a wide display the mask has more room than a 6" panel needs,
and on a narrow one both halves are cramped. It also means the window rearranges itself
when the mode changes, which a user has to learn once.
