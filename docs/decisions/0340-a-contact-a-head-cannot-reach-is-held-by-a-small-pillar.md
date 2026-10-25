# 0340. Hold a contact a head cannot reach, and a short run between faces, by a small pillar square on both

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A contact whose head has no room (ADR 0339) is left out, and automatic runs on figurines then
left islands in armpits, under chins and in folds of cloth that the check named and no hand
could support either. A small pillar fits where a head does not, but three rules made a
support between two faces of the part worse than one: tips close together merged into one
full-width support standing on the part, a joint that has to be cut away; every small
pillar with a neck was run into a support in its reach (ADR 0251), so one a millimetre over
the part became a thin strut leaning 7 mm across to a trunk; and its plain ends, square to
its axis or laid on a sloping face, met that face along an edge or as an ellipse several
times the diameter. Resin wires about 35 diameters long start to wave as they print, and a
run of 10 mm at 0.24 mm across is past that.

## Decision

- **A headless contact.** In a profile with small pillars, a contact with no room for a head
  is held by a small pillar or, failing one, by one strut straight down onto a face of the
  part looking up at it, at most 6 mm under it; a trunk from the contact would meet its face
  askew. A support left unsound by every other means, and any tip that cannot stand, is
  offered the same search before that strut.
- **The search.** From one diameter out along the contact's normal it looks along the normal,
  straight down, and 16 ways round at 15, 30 and 45 degrees off straight down, for a face of
  the part looking back at it, no steeper than 75 degrees and not painted out. Of the runs
  whose struts lean no more than 45 degrees and that are sound (ADR 0337) it takes the one
  that turns least at its joints, then the shortest.
- **Square on both faces.** Each end of a small pillar on the part ends in a half ball of its
  own diameter, centred its depth into the face, as a plane tip does (ADR 0339). An end
  meeting its face more than 20 degrees off the normal first runs out one diameter along the
  normal and bends there; within 20 degrees the mark is at most 6 % longer one way than
  round, under a pixel at 0.24 mm.
- **Straight where it can be.** Once every support is grown, a small pillar on the part is
  replaced by what the search finds from its contact where that turns at least five degrees
  less; what is kept is squared off its faces where that is sound and leans no strut past 45
  degrees.
- **Split rather than merge on the part.** A merged front that stands on the model is split
  back into one tree per tip where every tip stands on the model as a small pillar of its own.
- **Into a support only when nearer.** A small pillar is run into a support beside it
  (ADR 0251) only where the strut there is no longer than its own drop to the part.
- **Slenderness.** A support is thinned to a small pillar only within the profile's longest
  run and 30 of its diameters; a longer one keeps the regular width (ADR 0255).

## Consequences

Contacts in crevices with a face of the part in reach are held instead of reported, short
supports between two faces come off as thin struts, none crossing the model, and every small
pillar leaves a mark within 6 % of round. A contact over open air with only walls beside it
still stands nowhere. More small pillars stand on the part than on a support, so the part is
marked at both ends of each; reopen if those lower marks are reported, or if small pillars
break on prints at runs under 30 diameters. The search costs up to 50 rays and as many
soundness checks, for a contact nothing else stood and once for each small pillar on the
part; a split front is grown again per tip, only where it happens.

## Alternatives considered

### Drop a contact a head cannot reach

It leaves the face unheld in exactly the places that need holding.

### Keep merging, and build the merged trunk on the part thin

A trunk carrying several tips is what a thin strut cannot hold, which is why ADR 0042 keeps
the small pillar to one tip.

### A thicker small pillar for a longer run

A wider body needs a taper down to the diameter the profile asks the part to be marked with,
which is a head again, and the head is what did not fit.

### The option that won, and what it costs

Up to two more bends per small pillar meeting a face far off its normal, a second search and
soundness check for every small pillar on the part, a split forest with more struts to brace
and cut away, and a part marked at both ends of a strut it did not have before.
