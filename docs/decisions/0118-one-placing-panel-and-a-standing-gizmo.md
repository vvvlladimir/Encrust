# 0118. One placing panel, and a gizmo that is always out

- **Status:** Accepted
- **Date:** 2026-09-28

## Context

ADR 0105 made Move, Rotate and Scale a segmented switch in the Select panel, each showing
its own three numbers and its own handles. Placing a model takes all three in turn, so the
user flipped the switch back and forth, and the Select tool drew no handles at all. The
slicers our users come from show position, rotation and size at once, with move and rotate
handles on the pick and brackets on its bounds.

## Decision

`Tool::Move`, `Rotate` and `Scale` and their keys are gone. The Select panel stacks Move,
Rotation and Scale, each with a reset in its heading: position with Center and On
Platform, rotation with ±45° turns, Orient to Face — the next click lays the face under it
on the plate — and Auto-orient, scale as size in millimetres and ratio
in percent, linked by default, with Scale to Fit. While Select is out the gizmo stands on
the pick with arrows and, inside them, smaller rings, and each picked model's world bounds get corner
brackets and the length of each side.

## Consequences

- Everything about where a model stands is on one screen, and a drag needs no mode.
- Scale has no handles; it is typed. Size is the model's own axes stretched, not its
  world bounds, so a typed size survives a later turn.
- W, E and R are free. The Select panel is taller and scrolls on a short window.

## Alternatives considered

### Keep the switch, draw all handles in Select

Fixes the missing gizmo but keeps three screens for one job, and translate, rotate and
scale handles at once crowd each other on a small model.

### The option that won, and what it costs

Arrows and rings share one centre, so on a small model at a distance a ring can take a
press meant for an arrow. Stretching along one axis by dragging is no longer possible.
