# 0243. Offer a plane or a sphere tip, and bite a plane tip by pushing it in

- **Status:** Accepted; a bite of nought is allowed by 0323, the flat disc is superseded by 0345
- **Date:** 2026-10-13

## Context

ADR 0042 offered three tips. A cone ended in an apex `contact_depth_mm` past the contact,
a point that holds by a sliver of cured resin and is the first thing to tear off on the
peel. A plane ended on a flat disc at the surface and ignored `contact_depth_mm`, so the
Bite field did nothing for it, and a disc that only touches the face cures as a separate
solid and falls away. The two default presets shipped the cone.

## Decision

`ContactShape` is `Plane` or `Sphere`, and `Plane` is the default; the shipped Light and
Medium presets are `plane`. A file naming `cone` is a parse error like any other unknown
value: Encrust is in alpha, with no profiles in the field to carry across.

A plane tip is moved whole along its own axis `contact_depth_mm` into the model: its
contact disc lies that far under the surface, and the cone from it to the pillar is
shifted by as much. A trunk standing on the model ends the same way, upside down. The bite
is never longer than the top segment. The sphere is unchanged.

## Consequences

Every regular support now grips by a disc of the contact's full width sunk to the bite,
so it holds as the field number says and leaves a round scar of the contact's size. The
Bite field means one thing for both shapes. The neck outside the model is shorter by the
bite, since the cone moves in with it. A profile saved with a cone tip has its shape
edited by hand before it loads; if Encrust ships before a format freeze, reopen that.

## Alternatives considered

### Keeping the cone

It leaves the smallest mark, but its grip is the apex alone; a support that tears free
mid-print costs the whole part, and the sphere covers the case of a firmer hold.

### Sinking only a cylinder of the contact's width under an unmoved cone

The cone would keep its full length outside, but the bite would be a step in the
outline that the slicer prints as a ledge, not the shape a user reads off the Bite field.

### The option that won, and what it costs

A plane-tipped support scars every face by the full contact disc, where the cone left a
pinprick; a user after the finest finish lowers the contact diameter instead.
