# 0131. Ship the field's measurements, and the two rules they broke

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

The three shipped support profiles were our own numbers, sized against nothing in
particular and marked UNVERIFIED in the assets since step 7. The reference set for an
ELEGOO Saturn 4 Ultra — a light, a middle and a heavy profile — is what most resin printers
in the field are actually running, and a user carrying settings across expects ours to
start where theirs did.

Two of those numbers do not fit what the code already assumed.

**The contact is wider than the neck under it.** The reference middle profile drives a
0.8 mm contact into the face and necks down to 0.55 mm before widening to 1.2 mm — a nail.
`SupportProfile` validated a single non-decreasing chain from the contact down to the pad,
so that profile was rejected, and the Supports window's `keep_in_order` would have widened
the neck back to 0.8 mm the moment anything else was edited.

**The lattice was spaced off the head.** `SampleConfig` read `spacing_mm` as the head's own
carry times three: a 0.4 mm head gave 5 mm, and the reference's 0.8 mm head gave 8.3 mm. So
adopting the wider contact halved the support count — on the 30 mm ball the integration
test stands, 8 contacts fell to 2 standing. The reference does not derive the two from each
other at all: contact spacing is its own field, 4 mm in all three profiles, and the head is
free to be as fat as the part needs.

## Decision

**The shipped Light, Medium and Heavy profiles take the reference measurements for the
Saturn 4 Ultra**: the tip, the top segment, the pillar, the pad, the flare between them,
the small pillar, the raft and the cross bracing, which stays on. Where the reference
carries a measurement we do not have, the nearest field takes it: its lower main diameter
becomes `max_trunk_diameter_mm`, and its main upper diameter becomes the pillar, widened to
the top segment's lower diameter where that is wider, because our pillar is one width.

Two fields stay ours. `density` and `max_overhang_deg` keep their per-preset tuning —
0.6/1.0/2.5 and 55/45/35 — where the reference's three profiles are identical at 50% and 45
degrees. Three presets that place supports identically are one preset.

**A contact may be wider than the neck under it.** It leaves the ordering chain, in
validation and in `keep_in_order` both. Every other pair is unchanged: a ring below a ring
is still never narrower.

**`contact_spacing_mm` is a profile field**, 4 mm by default, and `SampleConfig` divides it
by the root of the density. The head's carry keeps its other job, `min_overhang_mm`.

The foot shape is **not** taken: the reference stands all three on a skate and we stand them
on a round pad, because our clearance test beams a foot at its full radius whatever its
shape, so a 12 mm skate would be tested as a 12 mm disc.

## Consequences

A plate supported with the shipped Medium now gets the reference geometry, which is heavier
than what Encrust shipped before: a 0.8 mm contact where it was 0.4, a 12 mm pad where it
was 8, cross bracing on by default, and a 4 mm lattice rather than a 5 mm one.

Unit tests no longer measure against a shipped preset. `core_supports::tests::profile` is
its own fixture, with round numbers, 45 degree branches and no bracing, so retuning what
Encrust ships cannot silently move what a test pins down. The integration tests still use
the presets, because what ships is what has to stand.

The nail head is the one place a support's silhouette is not monotone. `sweep` already
handled it — the two rings sit at the same height and the band between them is the flat
underside of the head — so nothing in the mesher changed.

Reopen the foot shape when `foot_clear` learns a skate's real footprint. Until then a user
who wants a skate can pick it, and should expect it to be refused more often than it
deserves.

## Alternatives considered

### Keep our own numbers and take none of them

No churn, no test rewrites. Rejected because they were never measured against a print, and
the whole point of matching the field's field names (ADR 0042) was that its numbers would
carry across.

### Take the density and the overhang angle too

Fully faithful. Rejected because the reference's three profiles differ only in thickness:
our Light and Heavy would then place the same supports in the same places, and the only
thing picking Heavy would change is how much resin each one costs.

### Derive the spacing from the head, and clamp it

Keeps one field instead of two. Rejected because the clamp is the field: as soon as the
answer is "4 mm whatever the head", the head is not what is being asked.
