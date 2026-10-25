# 0249. Hold everything that carries a support to 45 degrees, branches apart, and give a stranded tip a strut

- **Status:** Accepted; the head's 45 degrees is superseded by 0339
- **Date:** 2026-10-13

## Context

A tip left its face along the face's normal (ADR 0125), whatever the normal was: under a
face a few degrees past vertical the head ran out sideways, and off a wall it fell a
quarter as much as it ran, 76 degrees off vertical. A knee and the lean towards a brace
used the profile's branch angle (ADR 0078, 0240), which the Heavy profile sets to 70
degrees. A strut that flat is printed as an overhang of its own and sags.

The same flat head is one of three ways a contact ended up with nothing built under it: a
neck level with its contact had the model beside it and no column could leave it. The
second was the clearance sweep of a trunk beginning at its very top, a head's length from
the face it holds, which a leaning head brings inside the clearance. The third was a
contact in a crevice or under a low ledge, with no room for a head and a column at all.

## Decision

- A head leaves along the normal only while that falls at least as fast as it runs out;
  otherwise it runs the way the normal points and falls as fast: 45 degrees. A wall and a
  face that looks up are left the same way.
- A head that would meet the model within its own length goes straight down instead,
  where that is open.
- A knee, the lean towards a brace and the strut into a thicker support (ADR 0248) lean by
  the profile's branch angle and never past 45 degrees. A branch between two tips keeps
  the profile's angle.
- A trunk's clearance sweep begins its own width and clearance below its top.
- A front that can neither stand nor lean has each of its tips run on one strut from the
  contact straight down onto the model under it, within 6 mm and where the profile lets a
  support stand on the model: a tree of the one node, meshed as a small pillar.

## Consequences

No strut but a branch is flatter than 45 degrees, and on a lumpy test body the contacts
left bare went from 4 to 6 in a run to none on a sound surface. A head is no longer
always square on a steep face: its disc meets the face at an angle, and the bite is
measured along the head. A trunk may now pass within its clearance of the face it stands
under for its first millimetre. A contact with the plate under it and no clear way down
is still left bare, and the panel still counts it.

## Alternatives considered

### Marking a bare contact on the model

Tried first: a ring where nothing stood. It says where the problem is and leaves the user
to solve by hand what the placement can solve.

### Capping branches at 45 degrees too

A branch is short and tied at both ends, and the profile's angle is the user's to set.

### The option that won, and what it costs

A support in a crevice is cured onto two faces of the part instead of one, and both leave
a mark.
