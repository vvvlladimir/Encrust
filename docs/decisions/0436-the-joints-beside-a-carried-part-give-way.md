# 0436. The joints beside a carried part give way before the carry is refused

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

A carried part is drawn refused wherever `allowed` refuses it (ADR 0435). Grown trees put
every merge joint exactly at 45 degrees under its branches (ADR 0430), and the joint below
it the same way. A joint carried aside drops under its branches (ADR 0435), and that
tilts the strut down to the joint below past 45 degrees. So almost every step aside from a
merge was refused. The same happened when a strut was carried into the model while the
joint it meets could have stepped aside. The hand saw red where a small change to the
neighbouring struts would have let the support stand.

## Decision

- **`eased(before, after)` comes after `carried`.** If `after` is not `allowed`, it looks at
  each strut the carry bent out of line, between a node it moved and one it left. The node it
  left gives way if it is a joint, or the root while its foot stands on the plate (on the
  model, ADR 0437). Tips stay on the surface they hold, and a neck a head hangs from never
  gives way, since moving it would turn the head off its face (ADR 0435).
- **The nearest place that holds.** The joint is moved to the nearest of a bounded set of
  places: the place that brings each of its struts back to 45 degrees, rings of eight
  directions at 0.5 to 5 mm, straight down by those distances, and each ring place lowered by
  its own distance, none further than 5 mm from where the joint stood. It goes to the first
  place where every strut meeting it fits and still runs downward. The root also keeps its
  trunk clear and no shorter than a pillar, and its foot goes with it. At most four joints give way, each by at most 5 mm.
- **Judged as before.** The result must still be `allowed`; otherwise the carry stays
  refused, as ADR 0435 describes.
- **One rule for both drags.** The window eases every carried tree, and `tip_carried` eases
  what a tip carries.

## Consequences

A merge joint can be moved a few millimetres aside, and a strut moved against the model
pushes the joint beside it out of the way. What is held stays exactly where the hand put
it; only the joints next to it move. Only the first ring of joints gives way, so a carry
that would need the joint below a neighbour to move is still refused. A refused frame tries
up to about a hundred places per joint, each checking only the struts that meet that joint.
The cost grows with the struts carried, not with the supports on the model. If drags on big
trees stutter, the rings are the first thing to cut.

## Alternatives considered

### Re-grow the carried tree around what is held

This gives the best tree. But it is a run per frame, and every part the hand did not touch
would move.

### Give way down the whole tree

Each joint that moves could make the joint below it move too, all the way to the foot. This
helps more drags, but one drag could then reshape the whole support. The bound on retries
would also have to grow with the tree's depth.

### The neighbours give way, and what it costs

The support moves more than the hand asked: up to four joints, by up to 5 mm. The ghost
shows the eased tree, so the hand sees these moves before it lets go.
