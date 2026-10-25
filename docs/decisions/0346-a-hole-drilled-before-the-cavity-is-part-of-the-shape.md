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

`DrainHole::walled` marks a hole drilled into a model that was solid at the time;
`ModelHollow::add_drain` sets it from `is_hollow`, and the CLI sets it for every
`--drain-at` under `--drain-walled`. A walled hole is part of the shape:

- `sleeves` turns it into a `Blocker` from its mouth to its tip, its widest radius plus the
  wall, beside the channels' sleeves, so the cavity keeps a wall around it (ADR 0076);
- `pierce` leaves its depth alone;
- its plug is as deep as the hole, never cut down to the wall.

A hole drilled into a hollow model, or by `drill_into` into a pocket, is not walled and
drains as before. The field is serialised with a default of `false`, so a project saved
before it reads every hole as a drain, which is what it was cut as, and needs no new
format version. Clearing a walled hole from a hollow model moves its sleeve, so the shell
is built again at once, as clearing a channel does.

## Consequences

The order of Drain and Hollow now matters, and the drainage check is what tells a user
that a hole drilled first does not empty the cavity. A walled hole costs a hollow run where
it is added or cleared on a hollow model, as a channel does.

The sleeve is a capsule, so past a hole's flat end it leaves the hole's radius plus the
wall of solid on its axis rather than the wall alone; a few millilitres at most, and the
wall is never thinner than asked. A user who wants a hole drilled first to drain anyway is
the signal to reopen: it would need a way to turn the flag off on a placed hole.

## Alternatives considered

### Hollow the model with its holes already taken out

The field would be built from the drilled solid, so the wall would follow every hole
exactly. It needs the exact cut before the field, ties the cavity to a mesh that changes
with every click, and rebuilds the whole field rather than carving a kept one (ADR 0260).

### Wall every hole, whenever it was drilled

No flag and no order, but a hole would never drain unless typed deeper than the wall, and
every saved project would change what it prints.

### The option that won, and what it costs

A flag on a saved type and an order a user has to know about; the capsule's extra solid
past a hole's end.
