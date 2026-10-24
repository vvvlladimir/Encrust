# 0348. Hold a contact a head cannot reach with a small pillar, square on both faces

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

Since ADR 0346 a contact whose head meets the model is left out unless the model under it is
nearer than a head is long. Automatic runs on figurines then left islands in armpits, under
chins and in folds of cloth that the check named and no hand could support either: the
case 0346 named as its signal to reopen. A small pillar fits where a head does not, but it
only ran straight down, and its plain ends were laid on their faces (ADR 0331): on a sloping
face that is an ellipse up to several times the diameter, which marks the part as badly as
the slanted heads 0346 removed. Resin wires about 35 diameters long start to wave as they
print, and a run of 10 mm at 0.24 mm across is past that.

## Decision

- **First, room a step aside.** A contact whose head meets the model moves to the nearest spot
  of a face looking down within 0.3 mm across and 0.05 mm up or down where a head has room,
  looked for on rings 0.05 mm apart every 30 degrees: the reach a contact already moves onto
  an island by (ADR 0330), so what it held is still held. Placement picks a spot from the
  layers alone and cannot know the head will not fit there.
- **No room for a head, a small pillar.** In a profile with small pillars, a contact whose
  head meets the model there and a step aside is left headless whatever is under it. A headless contact is held by a
  small pillar or by nothing; a trunk from the contact itself would meet its face askew. A
  support left unsound by every other means is offered one too.
- **The search.** From one diameter out along the contact's normal it looks along the
  normal, straight down, and 16 ways round at 15, 30 and 45 degrees off it, for a face of
  the part looking back at it, no steeper than 75 degrees and not painted out. Of the runs
  whose struts lean no more than 45 degrees and that are sound (ADR 0341) it takes the one
  that turns least at its joints, then the shortest.
- **Square on both faces.** Each end of a small pillar on the part ends in a half ball of its
  own diameter, centred its depth into the face, as a plane tip does (ADR 0345). An end
  meeting its face more than 20 degrees off the normal first runs out one diameter along
  the normal and bends there; within 20 degrees the mark is at most 6 % longer one way than
  round, under a pixel at 0.24 mm, and a bend would cost more than it saves.
- **Straight where it can be.** Once every support is grown, a small pillar on the part is
  replaced by what the search finds from its contact where that turns at least five degrees
  less; what is kept is squared off its faces where that is sound and leans no strut past 45
  degrees. A head, a lean and a bend onto the face one after another are a zigzag a thin
  strut prints badly.
- **Slenderness.** A support is thinned to a small pillar only within the profile's longest
  run and 30 of its diameters; a longer one keeps the regular width (ADR 0255).

This supersedes laying a small pillar's ends on their faces in ADR 0331, and leaving out
a contact with no room for a head where the profile has small pillars in ADR 0346.

## Consequences

A contact picked a hair too near a wall stands on a regular support from beside it, as a hand
would put it. Contacts in crevices with a face of the part in reach are held instead of reported, and every
small pillar leaves a mark within 6 % of round. A contact over open air with only walls
beside it still stands nowhere. The search costs up to 49 rays and as many soundness checks,
for a contact nothing else stood and once for each small pillar on the part. A small pillar
is up to two diameters longer, so a few runs near the longest pass to the regular width.
Reopen if small pillars break on prints at runs under 30 diameters.

## Alternatives considered

### A thicker small pillar for a longer run

A wider body needs a taper down to the diameter the profile asks the part to be marked
with, which is a head again, and the head is what did not fit.

### Keep the plain ends and only search wider

It holds more contacts but leaves the ellipse on every sloping face.

### The option that won, and what it costs

Up to two more bends per small pillar meeting a face far off its normal, a second search
and soundness check for every small pillar on the part, and a part marked at both ends of
a strut it did not have before.
