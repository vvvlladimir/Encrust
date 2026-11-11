# 0439. A drag keeps the point taken under the cursor, and the aim is points of screen at the part's depth

- **Status:** Proposed
- **Date:** 2026-11-11

## Context

A carried support moved on the level plane through the middle of the part pressed (ADR
0438), however the part had been taken. A strut taken near its top was measured on a plane
millimetres below the point under the cursor. Seen from above at a slant, moving the mouse
up the screen then ran the part off in depth, toward the camera or away, faster or slower
than the cursor. After a refused drag the support also stood visibly off from where the hand
had pressed. The aim's slack was two points of screen measured where the camera looks (ADR
0244). A part nearer the camera was caught from further off on screen, and a part further
off barely at all: hover and press took the wrong part, or none.

## Decision

- **The plane passes through the point taken.** `grab_under_cursor` answers the point of the
  cursor's ray at the part, and the drag measures from it.
- **The plane is square to the camera.** What is held keeps its depth and moves the way the
  cursor moves on screen. With Alt it moves level with the plate instead, which is how a part
  is carried in depth.
- **The aim is six points of screen at the part's own depth.** `Slack` grows along the ray from
  where the camera looks to twice as far, so a part is caught as far off on screen however
  deep it stands. Misses are compared as the screen shows them, a part the ray goes through
  still winning.
- **The cursor says what a press would do.** An open hand shows over a part that would be
  taken, a closed hand while a carry has hold, and no-entry where the carry is refused.

## Consequences

The point pressed stays under the cursor for as long as the support may stand there. A drag
straight up the screen raises what is held, within what its branches allow: a joint never
rises above 45 degrees under them, and a foot never leaves what it stands on. Carrying in
depth needs Alt. The aim of six points takes a part the cursor is near, not only one it is
exactly on. Where two parts lie within it, the one nearer on screen is taken.

## Alternatives considered

### Keep the level plane through the middle of the part

The height stays put without a key. But depth swings with the camera's slant, and that was
the complaint.

### Measure misses in plate millimetres

That was the old rule, and it is simpler. But a millimetre is many points near the camera
and few far off, so what was taken depended on depth.

### The plane square to the camera, and what it costs

A stroke can lift or lower what is held as well as move it across. A part a hand wants to
carry level, it carries with Alt.
