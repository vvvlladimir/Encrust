# 0429. Stand a part near the plate on a short head: onto its pad, or cut into the plate

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

A part may hang a millimetre or less over the plate: a tail, a cape's hem, a model sunk
into the plate on purpose. A support under it had no way to be built. Its head is cut to
leave the shortest pillar (0.2 mm) over the pad, and the landing then refused a pillar of
exactly 0.2 mm, so nothing stood from 0.7 to about 2.7 mm up with the shipped pads — Grow
and a click alike, and the island printed loose. Where one did stand, the fillet from pillar
to pad, cut to the height there was, rose round the head as a bulge. Under the pad's height
and the shortest pillar there is no room for a foot at all.

## Decision

**The shortest pillar is a pillar.** A landing leaving exactly 0.2 mm stands.

**A fillet is drawn whole or not at all**, over the pillar under a head and never round the
head itself: a support whose pillar is shorter than its fillet's rise is a straight head on a
plain pillar into its pad.

**Under a pad and the shortest pillar the head runs into the plate.** `neck_along` ends the
head where its axis meets the plate; `landing` answers a `Landing` with `in_plate` set and
no pad. The mesher draws the tip and head as for any support, whole `[top]` length, cut flat
where it meets the plate; no foot, no fillet. `in_plate` is saved in `.encrust` with a
default of false.

## Consequences

A part 0.1 mm over the plate and up is held, by Grow, a click and Auto, which already put
contacts under such an island. A head cut into the plate holds on its own section there:
between the contact's and the pillar's width, under a millimetre across, which is what a part
that low weighs on. A contact under the plate, in a model sunk into it, still stands nowhere.
Reopen if heads cut into the plate tear off it on the first peel.

## Alternatives considered

### A thinner pad under a low part

A pad thinner than the profile's is not what the user set, and a blade still has to get
under it; the head alone is easier to cut off.

### Run a low head into the nearest pad

A pad stands no higher than the part; a head would meet it side on. Left for a part beside a
support, where it is reachable.

### The option that won, and what it costs

A new field in the saved file, and one more kind of landing every reader of `Landing` must
leave without a foot: the mesher and the fit of a dragged support do; a raft still takes such
a head into its outline and the head runs into the raft.
