# 0414. A hole set by hand stops at its depth

- **Status:** Accepted
- **Date:** 2026-11-09

## Context

Since ADR 0075 `pierce` carries every hole on a hollow model through the wall, so the depth a
user types is a floor. ADR 0346 exempted a hole drilled before the cavity; one clicked into a
shell still went through whatever was typed, and a recess in a shell could not be made. A search
does need the hole through: its proof is that the pocket empties.

## Decision

`HoleKind::AtDepth` is a hole placed at a point on a hollow model: a click, `--drain-at X,Y,Z`,
a recipe's `at = "points"`. `pierce` leaves its depth, and its plug is as long as the wall or
the hole, whichever is shorter, and Check counts it as a way out once its depth passes the wall.
`HoleKind::Drain` stays what `pierce` deepens: `holes_out`'s holes, those `drill_into` sends to
a trapped pocket, and every hole of a file saved before this.

## Consequences

A shell can carry a recess, and a saved project prints as it did. A hole typed shorter than the
wall no longer drains, which the drainage check reports; a script relying on the default 3 mm
through a thicker wall now has to set `--drain-depth`. A file with the new kind does not open
in an older build. A user wanting the kind of a placed hole changed is the signal to reopen.

## Alternatives considered

### Drop `pierce`, and let a search's depth stand

No new kind, but a saved project's clicked holes would become dimples, and a search's holes
would stop short when the model is hollowed again with a thicker wall.

### Mark clicked holes `Walled`

The kind exists, but it sleeves the cavity, so each click would rebuild the shell.

### The option that won, and what it costs

One more kind on a saved type, and a hole's kind now depends on who placed it as well as when.
