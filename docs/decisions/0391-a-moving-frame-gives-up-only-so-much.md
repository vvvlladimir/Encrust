# 0391. Let a moving frame give up only so much, and give up effects before faces

- **Status:** Accepted
- **Date:** 2026-10-31

## Context

ADR 0259 lets a frame drawn while the camera moves take any level to keep 60 Hz. The render
view (ADR 0390) made a frame dearer twice over: a fixed cost for the room and the set, and
four draws of every model for a clear resin. On a heavy plate the budget left for faces
fell towards nothing, and models turned into a few thousand broken faces while they moved,
the picked one included, and stayed so until the budget learnt in the other view caught up.

## Decision

- **A moving frame takes no step that shows more than 4 px** (`MOVING_MOST_PX`). Past that
  the frame is slower instead.
- **A picked model keeps at least a fifth of its faces** whatever the camera does.
- **Effects go before faces:** while the camera moves a clear resin is drawn opaque; the
  shadow map and the crease shading were already left for a resting frame.
- **The budget is learnt afresh when the render view is turned on or off.**

## Consequences

A model in motion always reads as itself; on a slow card with a heavy plate the frame rate
drops instead, and a clear resin pops to clear when the camera stops. The cap is a number
to revisit if a card that cannot hold it at 20 Hz turns up commonly.

## Alternatives considered

### Drawing every model whole while it moves

Never coarse, but a plate of millions of faces turns at a few frames a second on the cards
ADR 0259 was written for.

### Lowering the render resolution while the camera moves

Kept for later: it cuts the pixel cost the render view added, not the face cost, and blurs
every line of the window's marks with it.

### The option that won, and what it costs

A capped frame can miss 60 Hz by as much as the plate is heavy, and the cap is a guess at
what reads as the same model, not a measurement.
