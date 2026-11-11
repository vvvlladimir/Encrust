# 0437. Let no edit leave a strut flatter than forty-five degrees

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

Growth leans no strut under a head past `MAX_LEAN_PER_DROP`, 45 degrees, and stands a trunk
on the plate straight over its foot. A dragged foot keeps its trunk upright and leans what
it carries down to it at 45 degrees, but the knee never goes below the shortest pillar over
the pad: a foot dragged far enough laid the strut from the old knee to the new one almost
flat along the plate, and `fits` let it stand.

## Decision

**`fits` refuses a tree with a strut under its heads leaning past 45 degrees, or with a
trunk on the plate off the vertical.** Heads keep the lean of the face they hold, and a
trunk ending on the model the lean growth gave it.

**A dragged foot stops where a 45-degree lean from the knee reaches**, so dragging further
leaves it there rather than refusing the drag.

## Consequences

- Nothing a drag builds leans further than what Generate builds.
- A node, strut or trunk dragged into a flatter lean is refused like one dragged into the
  model, and slides by its run or its rise alone where that keeps within the angle.

## Alternatives considered

### Clamp every drag to the angle

A node dragged sideways would move by part of its step, unlike a node dragged into the
model; one rule for both is simpler to read.

### Refusing, and what it costs

A support cannot be laid flatter by hand even where the user means to.
