# 0238. Give a support one width, round its joints, and let a raft lift the part

- **Status:** Accepted; the dome at the end of every strut is superseded by 0352
- **Date:** 2026-10-12

## Context

A profile carried five widths for one column: the contact, `top.upper_diameter_mm`,
`top.lower_diameter_mm`, the pillar and `bottom.upper_diameter_mm` (ADR 0042, 0130). The
three in the middle could only repeat the contact and the pillar or draw a step: a neck
wider than its contact, a segment narrower than the pillar under it, a flare that began
wider than the pillar and so looked cut off. Three fields in the panel for a shape nobody
wants.

A joint was covered by a ball 1.15 trunks wide (ADR 0044), because a strut cut square
across its own axis cannot hide its end inside a trunk of the same width. The ball is extra
geometry on every merge and a bulge on every layer it crosses. `sphere-30` with Medium
printed a 0.02 mm2 island, which is gone with the ball and the stepped neck.

`z_lift_mm` sat in the raft's tab, and a raft lifted nothing: with 2 mm of lift and a
1.5 mm slab, the slab was printed into the part.

## Decision

**The cone under a tip runs from the contact's width to the pillar's, and the flare leaves
the pillar at the pillar's width.** `top.upper_diameter_mm`, `top.lower_diameter_mm` and
`bottom.upper_diameter_mm` are gone; a file that still names them loads, and they are
ignored.

**A strut is a tube of its own width that ends in a dome of that width on the node, and
the tube leaving the node starts under a dome of its own.** A trunk is never thinner than
what merges into it, so every end lies inside what it meets. The covering ball,
`JOINT_SWELL` and the widening of a strut towards its trunk are gone.

**A raft stands the part up by its own thickness, on top of `z_lift_mm`.** The command
line adds the two. The window raises every supported model when the raft is switched on
and lowers it when the raft or the supports go; how far it was raised is kept beside the
model's supports and in the project as `raft_lift_mm`. The Lift field and its button are
the lift alone.

## Consequences

- One width per support in the panel; a preset tuned with a wide neck loses it.
- A joint is as wide as the trunk under it and no wider, whatever the angles and however
  many struts meet; where the trunk is thicker than a branch its top shows as a dome.
- A branch stays as thin as its tip until it merges, where it used to widen on the way.
- A model moves when its raft is switched on. A project written before this reads as one
  no raft has lifted, so opening it with a raft on raises its supported models once.
- Reopen if branches snap above a merge: the answer is a taper, not a ball.

## Alternatives considered

### Keep the widths and hide them

Costs nothing in the format and keeps every way of drawing a step alive in files nobody
can edit from the window.

### Extend each strut into its trunk

ADR 0041 did, and ADR 0044 records why it fails: a strut as wide as its trunk comes out of
the far side.

### Cut every end level with the plate

Tried first: struts and trunk meet on the node's horizontal plane. An end as wide as the
trunk it widened into stood out of it as a flange, and the widening itself read as a cone
between two joints.

### Count the raft into the Lift button

One number to press, and a label that disagrees with the field beside it.

### One width, domed ends and a raft that lifts, and what they cost

A nail-headed tip — a neck thinner than both its ends — can no longer be drawn. Each end at
a joint costs two more rings. The window moves a model the user did not touch.
