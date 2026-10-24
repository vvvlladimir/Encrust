# 0255. Thin a support between two surfaces only up to a longest run

- **Status:** Accepted; held to 30 diameters by 0348
- **Date:** 2026-10-13

## Context

A support that lands on the model with one tip is built as the small pillar of
`[small_pillar]`: one thin width from the face it holds to the face it stands on (ADR
0042). Which supports qualify was decided by where they land and how many tips they carry,
never by how long they are. A 0.3 mm strut across a 2 mm gap under a ledge is what the
small pillar is for; the same strut run 15 mm from an arm down to a leg sways on the peel
and snaps, and the overhang it held prints loose.

## Decision

**`[small_pillar]` carries `max_length_mm`**, the longest run, contact to landing along its
struts, that is thinned down. A longer support between two surfaces keeps the regular
pillar's width and tip. Absent from a file it reads as 10 mm, and every shipped preset
carries 10 mm. It must be above zero.

The rule sits where a small pillar is recognised (`mesher::Style::of`), so the mesher, the
running of small pillars into a thicker support (ADR 0251) and what a carrier is as wide as
all ask the same question.

## Consequences

- A long model-to-model support is wider than before and leaves a tip's mark rather than a
  pillar's on both faces.
- A profile saved before this loads unchanged in everything but that 10 mm limit.
- The Small pillar panel gains a Max length field, and its drawing a dimension over the
  stretch between the walls.

Reopen if supports just past the limit snap anyway: the length may want to scale with the
pillar's diameter rather than stand on its own.

## Alternatives considered

### Drop a support whose run is too long

Nothing thin and long is ever built. Rejected because the overhang above it is left with
nothing under it; a wider support is still a support.

### Derive the limit from the diameter

No new field. Rejected because how far a thin strut holds depends on the resin as much as
on its width, and a number the user can set is the one they can tune on a print.

### The option that won, and what it costs

One more field per profile, and one more number a user has to understand on the panel.
