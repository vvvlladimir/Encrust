# 0074. The section cap counts material, not crossings

- **Status:** Accepted
- **Date:** 2026-09-23

## Context

The cutting plane is capped through a stencil: crossings above the plane are counted, and
the cap is painted wherever the count says the plane runs through a solid (ADR 0062). That
rule was written when everything on the plate was wound outward.

A drain hole is not. It is a closed body appended wound inward (ADR 0071), and it reaches
out past the surface it is drilled in, by `lift_mm` wherever something stands over the
mouth. Two things then paint a cap in mid-air:

- The counting pass subtracted the hole per fragment, as the model pass does (ADR 0073).
  A cylinder cuts the outer surface and the cavity in circles that do not match, so the
  sliver between them lost one crossing of its pair and the count never came back.
- A tube standing in the air across the plane counts `+1` rather than `-1`, and the old
  test asked only whether the count was non-zero.

Both painted the cap colour where the plane misses the model entirely.

## Decision

The count is signed, and material is where it ran **negative** — a ray that entered more
solids than it left. The cap's stencil test is `Less` against `MATERIAL_ABOVE`, half the
stencil's range, which negative counts wrap past.

The counting pass subtracts nothing. A hole is already wound the other way round, so it
takes itself back out of the count: inside the wall the shell's `-1` and the hole's `+1`
cancel and the cap opens over the bore, and in the air the hole alone counts `+1`, which
is not material.

## Consequences

The stencil now does the same arithmetic as the fill rule, on the same geometry, so the
cap and the printed layer agree by construction rather than by two rules being kept in
step. The model pass still subtracts per fragment — that one draws the hole, and has no
count to work with.

Up to 127 solids may overlap at a pixel before the count wraps into the other sign. A
plate of a model, its lattice and its supports uses a handful.

`crates/encrust-app/src/render/section_cap.rs` renders the viewport offscreen and holds the
invariant: drilling a model paints nothing where the model was not. It needs an adapter and
does nothing on a machine without one.

## Alternatives considered

### Keep subtracting in the counting pass and widen the skin

Chases the sliver with a tolerance. The mismatch is between a cylinder and two curved
surfaces, so there is no tolerance that closes it without eating the cap around every hole.

### Clamp `lift_for` so a mouth never stands far out

Hides these two failures for holes on a gentle surface and leaves them for holes drilled
under something. The lift is not the bug.

### The option that won, and what it costs

The cap now depends on the winding of every mesh that reaches the viewport, not just on
its being closed. A model imported inside-out and drawn without `orient_outward` would cap
as air. Both binaries orient on import, and the fill rule has depended on the same thing
since ADR 0071, so this is one more consequence of a decision already taken rather than a
new exposure.
