# 0430. Lead no branch back the way it came, and stand a dragged trunk upright

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

`grow` merges the highest meeting point first and puts each joint where its first two branches
meet (ADR 0040). A third met later beyond one of them leads that branch back the way it came:
on a crab figure 32 tip paths turned back past square below their heads, long thin struts
running round the part and back. Dragging a foot by hand moved only the landing, so the trunk
leaned from its joint to the foot at whatever angle the drag left, past 60 degrees off a part,
printed as an overhang.

## Decision

**Once merging is done, `Forest::shortcut` runs every branch a joint leads back** (turning past
square across the plate) straight on to the joint under it, where that strut leans no further
than every group's branch angle, runs no longer than its `max_strut_length_mm` and clears the
model with its clearance; a joint left with one branch is straightened the same way, and one
left with none is dropped. Joints are taken top down, once each.

**A dragged foot keeps its trunk upright.** `knee_over_foot` stands the root straight over the
foot at the height the branch above reaches it at 45 degrees, no lower than a pad and the
shortest pillar: the knee that already stood over the foot moves with it, otherwise one is
put in. Dragging a trunk whole still moves it as one.

## Consequences

On the crab, paths turning back fall from 32 to 8, the trees and every standing tip the same;
on the bench resin rises up to 5 per cent, a branch cut loose from a joint no longer sharing
its strut, and one tip of 335 on the hollow duck is lost where a tree that already failed its
bodies is mended into other halves. A dragged support may now be refused where its knee meets
the model. Reopen if prints break at the struts this lengthens, or the duck's tip matters.

## Alternatives considered

### Refuse a merge that leads a branch back

Tried: three tips in a row no longer shared a trunk; 35 per cent more trunks and 33 per cent
more resin on the crab.

### Move each joint to where its struts are shortest

Tried, a weighted median held to the lean: a joint where three branches from three sides meet
is already near that point, and the paths still turned back; the topology, not the place, is
what leads them back.

### The option that won, and what it costs

A branch shortcut no longer shares the strut under its old joint, which is the resin above,
and the shortcut is checked as a merge strut is, not as the drawn bodies are.
