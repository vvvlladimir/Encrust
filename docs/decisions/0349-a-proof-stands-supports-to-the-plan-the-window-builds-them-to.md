# 0349. A proof stands supports to the plan the window builds them to

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

A proof (ADR 0277) stands supports under every finalist and reads their resin. `Trial`
carried one `SupportProfile`, and the window handed it the active group's. With an automatic
profile chosen (ADR 0325) the window builds supports differently: it places them by the
automatic profile's own spacing and overhang angle, then builds each one to the light,
medium, heavy or steadying shape that what it holds calls for. The proof built every support
to the group's profile instead, so with sizing on, **Auto-orient** reported less support
resin than the supports the window then built, and judged the fewest supports on those wrong
readings.

## Decision

`Trial::support` is a `core_engine::SupportPlan`, the same plan `stand_under` and
`stand_seated` take. The window builds it with `SupportTool::orient_plans`: with an
automatic profile chosen, `SupportShapes::Sized` with the shapes the catalogue names, as
`SupportTool::start` places them; otherwise the active group's profile. Every shape stands
on the model's raft, its own or the plate's, and at the lift the user typed (ADR 0291),
set through `SupportPlan::profiles_mut`. The search reads its overhang angle, contact
spacing and lift from `SupportPlan::placing`. The command line, which builds supports to
one profile, passes `SupportPlan::of` that profile.

## Consequences

A proof's support resin is what the window's own run builds for the same pose, sizing
included. Choosing an automatic profile now changes what **Auto-orient** chooses, as it
changes the supports. A hollowed model's cavity is still not supported in a proof
(`cavity: false`). Reopen this if the proof should also weigh supports inside the cavity.

## Alternatives considered

### Hand the proof the automatic profile's placing profile alone

It would fix the spacing and the angle, but still build every support to the lightest shape,
so the resin would still read low wherever sizing makes one heavier.

### The option that won, and what it costs

The window's orient call needs the catalogue to find the shapes, and fails if a shape the
automatic profile names is missing, just as a support run would.
