# 125. Leave the surface along its normal, and merge from the neck

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

A tip was drawn along the axis of whatever carried it: a vertical column entered a flat
ceiling square on, but the moment branching pulled the strut sideways, or the face leaned,
the cone met the surface at a glancing angle. That is a long thin scar instead of a point,
less grip per millimetre of contact, and it is not what the tip's measurements describe.
Other slicers stand the tip on the face's own normal and only then turn.

The contact carries no normal of its own: points come from the slice stack, from a brush
or from a click, and `SupportPoint` is in the project file.

## Decision

`columns` reads the normal of the face nearest the contact through the model's `Bvh`, and
puts a **neck** one `top.length_mm` out along it. The neck is where the body begins:
`landing` drops its column from the neck, and `grow` seeds its fronts there, so merging,
knees and every clearance beam work from the neck down. A tip is therefore always the
`[tip]` and `[top]` segments square on the face, whatever the branch under it does.

The normal is used whenever it points downwards at all, and straight down otherwise. The
neck is shortened where a full one would eat the room the body needs above the plate.

The bite follows: a tip's apex is `contact_depth_mm` back along its own axis rather than
straight up, which supersedes that clause of ADR 0028. The rest of 0028 stands — the
contact is still remembered in the model's own space, and the body under the neck is still
vertical, because the peel pulls along the plate's up and not along any face.

Two nodes where there was one means a seam at the neck. A node the strut runs straight
through wears no ball: the strut above reaches `JOINT_OVERLAP_MM` past it instead, so a
plain vertical column keeps the silhouette it had.

## Consequences

The tip's numbers now mean what they say on every face, and the scar is round. The body
starts up to one `top.length_mm` to the side of the contact, so on a 45 degree face the
pillar stands about 1.4 mm off the point it holds — more room found on some parts, less on
others, and the clearance beams already say which.

The neck also spends `top.length_mm` of the height a merge has to happen in, so in a gap
only a few millimetres deep fewer tips find a shared trunk than before — twelve supports
under a 14 degree wedge 5 mm tall came back as twelve trunks where they were eight. How
many stand is unchanged.

Nothing is checked over the neck itself, exactly as nothing was checked over the head it
replaces: a 2 mm tip out of a concave surface can still re-enter it. Reopen if tips start
grazing.

`Body` is gone: with the body starting at the neck there is no head left to exempt, and
`landing` takes a radius.

## Alternatives considered

### Turn the tip at mesh time only

Keep one node and draw the head along the normal in `mesher`. The tree would then lie
about where the body is, and merging, landings and clearance would all carry on from the
contact — the geometry and the analysis would disagree.

### A neck of its own length, from a new profile field

More control, and a second knob that only means anything next to `top.length_mm`, with a
seam inside the cone whenever the two disagree.

### The option that won, and what it costs

Every tip is two nodes, so trees are twice the size they were and a branch turning at the
neck wears a ball there. It is the knuckle other slicers draw in the same place, but it is
resin, and it is one more cover to slice.
