# 0438. A carried support follows the cursor, and stands only where it is allowed

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

A drag carried what was held by the step the cursor took on a plane facing the camera (ADR
0244). One stroke of the mouse moved a part across the plate and up at once, mixed by the
camera's angle. A step `fits` refused was dropped, or tried again as its run or its rise
alone (ADR 0437). So the support stopped or slid along one axis with no warning. A merge
joint stands exactly at 45 degrees under its branches (ADR 0432), so any step aside was
refused, and so was every step of a support that did not fit before the drag. Each frame
also dropped what was built and meshed every support of the model again. With hundreds of
supports, the drag stuttered.

## Decision

- **The plane.** A drag carries across the plate on the level plane through the part pressed.
  With Alt held it carries up and down instead, on an upright plane turned to the camera.
  Pressing or letting go of Alt mid-drag starts the measure again from where things stand.
- **What is held follows the cursor and never stops.** Every frame carries from how the
  supports stood at the press, by where the cursor is on that plane less where it was
  pressed. The result is drawn whatever the model says: in its group's tint where it may
  stand, in danger's with a no-entry cursor where it may not. The scene takes the result only
  where it may stand, so a release leaves the support where it last could stand.
- **Making a move is apart from judging it.** `carried` and `followed` build the moved trees.
  `allowed(before, after)` judges them: the result `fits`; or, when `before` did not fit,
  every strut and trunk the carry moved keeps out of the model and leans no more than 45
  degrees. A support from an older build can then still be carried.
- **The constraint is solved, not refused.** A joint carried aside drops to the height its
  branches allow at 45 degrees. A column raised stops at that height, and lowered stops at
  the shortest pillar.
- **The column is one handle.** The root, the trunk and the foot carry together, and so does
  a strut hanging from the root. Across the plate its foot moves, its trunk stands upright
  over it, and what it carries leans down to a knee at 45 degrees. It stands on whatever
  `landing` finds there, the model or the plate. A foot never rises. Any other joint or strut
  moves its own nodes. A tip held alone is still aimed onto the surface (ADR 0244).
- **What is carried is lifted.** `ModelSupports::lift` leaves the carried trees out of the
  meshes, built once when the drag starts and once when it ends. On each frame `carry_to` puts
  them back without building anything, and the window meshes only them as the ghost.

## Consequences

The same stroke does the same thing from any camera, and nothing jumps: the ghost is always
under the cursor, and taking the cursor back takes the supports back. A frame costs one
`carried`, one `allowed` and one mesh per carried tree, whatever the model holds. A support
can be dragged through a place it may not stand to reach one it may. Seen edge-on, a plane
gives no crossing, so with the camera level with the plate a drag waits until the camera
tilts. A support from an older build that does not fit stays as it is except where it is
carried. A part no longer slides along an obstacle, as ADR 0437 described.

## Alternatives considered

### Refuse the step and halve it toward the cursor

This was this record's first draft. What is held stops short of the obstacle, but it
stopped dead on every joint at 45 degrees and every support that already failed. Up to six
`fits` a frame made the stutter worse.

### Slide along the obstacle

This keeps what a part touches under the cursor. But it moves the part on an axis the hand
did not ask for.

### Let it stand where it was let go, marked as refused

This is the most freedom. But it leaves supports in the model that slicing would print.

### Following the cursor, and what it costs

When the cursor is over a refused place, the ghost and the support part ways: the ghost is
under the cursor, and the support waits where it last could stand. The hand sees both: the
red ghost, and the picked outline where the support will stay.
