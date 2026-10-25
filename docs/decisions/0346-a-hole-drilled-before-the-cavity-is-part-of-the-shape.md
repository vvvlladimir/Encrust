# 0346. A hole drilled before the cavity is part of the shape

- **Status:** Accepted
- **Date:** 2026-10-25

## Context

Since ADR 0075 a hole is cut on its own and `pierce` carries every hole on through the wall
once the model is hollow, whatever depth was asked for, and ADR 0321 cuts its plug to that
wall. That is right for a hole
drilled into a shell to empty it. It is wrong for one drilled into the solid model first:
the user cut a recess of a stated depth, took its plug, and then hollowed. The hollow run
knew nothing of the hole, so the cavity ran up to and into it, the hole was cut down to the
wall, and its plug was cut again from a 12.6 mm cylinder to a 2 mm disc.

## Decision

`DrainHole::kind` says what a hole is for; `HoleKind::Walled` marks one drilled into a model
that was solid at the time. `ModelHollow::add_hole` sets it from `is_hollow`, and the CLI
sets it for every `--drain-at` under `--drain-walled`. A walled hole is part of the shape:

- `sleeves` turns it into a `Blocker` from its mouth to its tip, beside the channels'
  sleeves, so the cavity keeps a wall around it (ADR 0076). A blocker is now a flat-ended rod
  of `core_mm`, the hole's widest radius, grown by `wall_mm`: the wall is as thick past the
  hole's end as along its side. The core scales with the model and the wall does not, so it
  is the shell's thickness at any scale, a channel's sleeve too. A file without the fields
  reads the capsule it was;
- `pierce` leaves its depth alone;
- its plug is as deep as the hole, never cut down to the wall.

A hole drilled into a hollow model, or by `drill_into` into a pocket, is a `Drain` and
drains as before. The field is serialised with a default of `Drain`, so a project saved
before it reads every hole as a drain, which is what it was cut as, and needs no new
format version. Clearing a walled hole from a hollow model moves its sleeve, so the shell
is built again at once, as clearing a channel does.

## Consequences

The order of Drain and Hollow now matters, and the drainage check is what tells a user
that a hole drilled first does not empty the cavity. A walled hole costs a hollow run where
it is added or cleared on a hollow model, as a channel does.

A tapered hole is walled as a cylinder of its widest radius, so the wall around its narrow
end is thicker than asked by what the taper took in. A user who wants a hole drilled first
to drain anyway is the signal to reopen: it would need a way to change a placed hole's kind.

## Alternatives considered

### Hollow the model with its holes already taken out

The field would be built from the drilled solid, so the wall would follow every hole
exactly. It needs the exact cut before the field, ties the cavity to a mesh that changes
with every click, and rebuilds the whole field rather than carving a kept one (ADR 0260).

### Wall every hole, whenever it was drilled

No new kind and no order, but a hole would never drain unless typed deeper than the wall, and
every saved project would change what it prints.

### A capsule of the hole's radius plus the wall

What `sleeves` gave a channel already, and no new field. Past a flat end it leaves the hole's
radius plus the wall on its axis: 12 mm of solid under a 20 mm hole in a 2 mm wall.

### The option that won, and what it costs

A field on a saved type and an order a user has to know about, and two on `Blocker`.
