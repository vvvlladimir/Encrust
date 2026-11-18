# 0435. A carried support follows the cursor from the point taken, and stands only where growth would let it

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

A drag carried what was held by the step the cursor took on a plane facing the camera through
the middle of the part (ADR 0244). A strut taken near its top was measured on a plane
millimetres from the point under the cursor, so it ran off in depth faster or slower than the
cursor. The aim's slack was two points of screen where the camera looks: a part nearer the
camera was caught from further off, one further off barely at all.

Nothing held an edit to what growth builds. Growth leans no strut under a head past
`MAX_LEAN_PER_DROP`, 45 degrees, stands a trunk on the plate straight over its foot, and turns
no head past `MAX_HEAD_LEAN_DEG`, 33 degrees, off its face (ADR 0339, 0429). A foot dragged
far laid the strut from its old knee almost flat along the plate; a tip carried by hand turned
its head as far as the hand took it, and moved a neck another branch had come down onto; every
tip was pulled back onto its nearest surface, those the hand did not carry included. A step
that did not fit was dropped, so the support stopped with no warning, and each frame meshed
every support of the model again: with hundreds of supports the drag stuttered.

## Decision

- **What may stand.** `fits` refuses a tree with a strut under its heads leaning past 45
  degrees, a trunk on the plate off the vertical, or a head it keeps (same tip on the same
  neck) turned past 33 degrees off its face or further than it already was. A head a carry
  builds anew is growth's and held to growth's rules. `allowed(before, after)` judges a carry:
  the result `fits`; or, when `before` did not fit, every strut and trunk the carry moved keeps
  out of the model and within those leans, so a support from an older build can be carried.
- **The plane passes through the point taken, square to the camera.** `grab_under_cursor`
  answers the point of the cursor's ray at the part, and what is held keeps its depth and moves
  the way the cursor moves on screen. With Alt it moves level with the plate. Pressing or
  letting go of Alt mid-drag starts the measure again from where things stand.
- **What is held follows the cursor and never stops.** Every frame carries from how the
  supports stood at the press. The result is drawn in its group's tint where it may stand, in
  danger's with a no-entry cursor where it may not; the scene takes it only where it may stand,
  so a release leaves the support where it last could stand. An open hand shows over a part a
  press would take, a closed hand while a carry has hold.
- **The constraint is solved, not refused.** A joint carried aside drops to the height its
  branches allow at 45 degrees. A column raised stops at that height, and lowered at the
  shortest pillar. A dragged foot stops where a 45-degree lean from its knee reaches.
- **The column is one handle.** The root, the trunk and the foot carry together, and so does
  a strut hanging from the root: the foot moves, the trunk stands upright over it, and what it
  carries leans down to a knee at 45 degrees. Any other joint or strut moves its own nodes.
- **Only a carried tip is aimed back onto the surface** (ADR 0244). A tip carried off a neck
  it shares takes a neck of its own, hung from the old one, which is now a joint.
- **The aim is six points of screen at the part's own depth.** `Slack` grows along the ray from
  where the camera looks to twice as far; misses are compared as the screen shows them, a part
  the ray goes through still winning.
- **What is carried is lifted.** `ModelSupports::lift` leaves the carried trees out of the
  meshes, built once when the drag starts and once when it ends; each frame `carry_to` puts
  them back without building anything, and the window meshes only them as the ghost.

## Consequences

The point pressed stays under the cursor for as long as the support may stand there, the same
stroke does the same thing from any camera, and taking the cursor back takes the supports back.
Nothing a drag builds leans further or turns a head further than what Generate builds. A frame
costs one `carried`, one `allowed` and one mesh per carried tree, whatever the model holds. A
support can be dragged through a place it may not stand to reach one it may. Carrying in depth
needs Alt, and seen edge-on a plane gives no crossing, so the drag waits until the camera
turns.

## Alternatives considered

### Refuse the step, or slide it along its run or rise alone

The first drafts. What is held stopped dead on every merge joint, which stands exactly at 45
degrees (ADR 0430), and on every support that already failed, or moved on an axis the hand
did not ask for; up to six `fits` a frame made the stutter worse.

### A level plane through the middle of the part

The height stays put without a key, but depth swings with the camera's slant.

### Let it stand where it was let go, marked as refused

The most freedom, but it leaves supports in the model that slicing would print.

### Following the cursor, and what it costs

When the cursor is over a refused place the ghost and the support part ways: the red ghost is
under the cursor, the support waits where it last could stand. A support cannot be laid
flatter than 45 degrees by hand even where the user means to.
