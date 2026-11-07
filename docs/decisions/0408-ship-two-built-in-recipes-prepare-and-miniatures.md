# 0408. Ship two built-in recipes: prepare and miniatures

- **Status:** Proposed
- **Date:** 2026-11-06

## Context

The recipes of ADR 0403 shipped with four built-ins: `prepare`, `miniatures`, `large-solid`
and `flat`. Read side by side, they differed by a setting or two, not by strategy.
`large-solid` was `prepare` with a wall scaled to the height; `prepare` already hollows only
where a wall saves resin. `flat` stood a part on no supports, so most parts with an
overhang failed its island gate. A third candidate, functional parts, was `prepare` with
heavy supports, no hollowing and a closing pause, all of which the Prepare form already
sets. Each built-in costs its tests, panel pictures, translated names and lines in
`docs/cli.md` and `docs/features.md`, and a list of near-copies hides what each one is for.

## Decision

Two recipes ship. `prepare` is the autopilot of ADR 0402. It now stops with a reason where
resin stays trapped after draining, and where the model on its supports does not fit the
build volume. `miniatures` is the one other strategy: it repairs an open mesh, orients for
the least peel with the tilt limit in `[vars]` as `max_tilt` (45°), never hollows, uses Auto
supports, checks islands up to twice more and stops on any left. Both orient only a model
not turned by hand (`!turned`). A built-in recipe is added only for a different strategy,
never for a different setting.

## Consequences

The picker offers the default and the case resin printing is most often used for, each
plainly different. A user who wants heavy supports, a scaled wall or a pause copies
`prepare` and edits it. `miniatures` also shows `[vars]`, `retry` and a gate in a recipe a
user starts from. A recipe id a user saved as their choice that no longer ships (`large-solid`,
`flat`) is not found and has to be chosen again. Reopen this if users keep making the same
copy of `prepare`.

## Alternatives considered

### One built-in, `prepare` alone

The least to keep up, but least-peel tilting is a separate strategy, not a setting, and the
recipe language would have no shipped example of `vars` or `retry`.

### Three, with functional parts

Matched a kind of model by name, but its only differences from `prepare` were settings
the Prepare form already covers.

### Two, and what it costs

Large solid, flat and functional parts no longer have a recipe named for them; a user who
looks for one finds `prepare` and must know which settings to change.
