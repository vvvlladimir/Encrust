# 124. End a support that stands on the part in a contact, and let a profile refuse one

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

A trunk several tips have merged into lands wherever its front can stand, the model
included. Only a tree with one tip was thinned to the `[small_pillar]` strut; anything
wider came down at the full trunk radius — up to `max_trunk_diameter_mm` — and sank
`landing_depth_mm` into the face. A 2–3 mm column cured into the part cannot be cut off
without taking the part with it, which is what a branching run leaves inside a recess.

The top of a support already answers this: `[top]` narrows the body to the `[tip]` contact
and only that contact bites. The bottom had no such geometry.

## Decision

A trunk landing on the model, and not already a small pillar, ends in that same shape
upside down: a cone over `top.length_mm` from the trunk's own width down to
`top.upper_diameter_mm`, then `tip.contact_diameter_mm` driven `landing_depth_mm` into the
face. It leaves the trunk at whatever width the trunk is, rather than at
`top.lower_diameter_mm`, so a merged trunk has no rim hanging off the top of its cone.
Neither the tip count nor the trunk's width changes what touches the part.

`SupportProfile` gains `land_on_model`, on by default. Off, `landing` refuses a hit on the
model, and the tip falls to what a blocked front already does: merge into a trunk that can
stand, bend a knee for the plate (ADR 0078), or be dropped.

## Consequences

Every support on the part comes off by hand, and the scar is a tip's scar. The grip there
is a contact's grip, so a heavy branch on a shallow ledge can snap during the peel; the
answer is a wider `contact_diameter_mm`, or the plate.

The clearance beam is still swept at the full body radius down to the face, so it now
guards more than the geometry needs and may refuse a landing the narrowed end would have
fitted. Reopen if landings start going missing in tight recesses.

## Alternatives considered

### Cap the trunk's radius on a model landing

Shrinks the weld without removing it — a 1 mm stump cured into a face is still a stump —
and adds a knob that only means anything beside another knob.

### Refuse every model landing

Inside a recess there is no plate under the tip, so this prints unsupported overhangs. It
survives as the switch rather than as the default.

### The option that won, and what it costs

The cone is a stress riser at the end that carries the whole branch: it breaks where we
want it to, and sometimes when nobody asked. Its length is the segment's whatever the
trunk's width, so a wide trunk narrows over a steeper cone than a tip's, and a
`top.length_mm` longer than the trunk degenerates to a straight plug, closed but unwarned.
