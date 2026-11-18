# 0427. Stand a part near the plate on a short head, and fillet a trunk at its own width

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

A part may hang a millimetre or less over the plate: a tail, a cape's hem, a model sunk
into the plate on purpose. A support under it had no way to be built. Its head is cut to
leave the shortest pillar (0.2 mm) over the pad, and the landing then refused a pillar of
exactly 0.2 mm, so nothing stood from 0.7 to about 2.7 mm up with the shipped pads — Grow
and a click alike, and the island printed loose. Where one did stand, the dome of the joint at
its neck and the fillet under it rose round the head as a bulge; and every fillet ended at the
trunk's width at the pad, wider than a trunk that narrows going up, in a ledge. Under the
pad's height and the shortest pillar there is no room for a foot at all.

## Decision

**The shortest pillar is a pillar.** A landing leaving exactly 0.2 mm stands.

**A fillet meets what stands on it at that one's width.** A trunk narrows going up, so the
quarter circle ends where `r(top) + rise = lower`, solved in closed form, its top ring the
trunk's width there; with less room than that it is a smaller whole quarter circle, never one
cut off. It fillets the pillar under a head, never the head itself, and each level ring
stands on the trunk's axis at its height, so a leaning trunk keeps its fillet round it.

**A lone tip whose pillar is shorter than its head is one tube**, contact to foot or plate:
`mesher::drawn` draws it without the joint at its neck, whose dome stood round the head. What
is saved keeps its neck.

**Under a pad and the shortest pillar the head runs into the plate**, straight down whichever
way its face leans: `neck_along` ends it on the plate under its contact; `landing` answers a `Landing` with `in_plate` set and
no pad. The mesher draws the tip and head as for any support, whole `[top]` length, cut flat
where it meets the plate; no foot, no fillet. `in_plate` is saved in `.encrust` with a
default of false.

## Consequences

A part 0.1 mm over the plate and up is held, by Grow, a click and Auto, which already put
contacts under such an island. A head cut into the plate holds on its own section there:
between the contact's and the pillar's width, under a millimetre across, which is what a part
that low weighs on. A contact under the plate, in a model sunk into it, still stands nowhere.
On the bench every support still stands and resin falls 0.1–4 per cent, the fillets of
narrowing trunks lower and no dome at a short neck. Reopen if heads cut into the plate tear
off it on the first peel.

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
