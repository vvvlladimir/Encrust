# 0474. A lift is a placement on every front end

- **Status:** Proposed
- **Date:** 2026-11-23

## Context

The window stands a model off the plate by its **Lift off the plate** row, keeps the height on
the model, and Auto-orient stands a pose on supports at it (`OrientTask::of`, `lifted`). Since
ADR 0472 the command line grows supports under the model where it stands and lifts nothing
first, so a model drawn on the plate gets no supports under its floor and the command line has
no way to stand it higher. `Edit::Orient` carried no lift, so an orient carried out by
`apply` weighed its poses at the profile's lift whatever the user had asked.

## Decision

- **`--lift MM`** stands each model that high off the plate, last of the placing flags, by a
  `Move` of its box's floor to the height, as the window's row moves it.
- **`Edit::Orient` carries `lift_mm`**, optional: the height a pose on supports stands at, the
  support profile's lift where unset. `--lift` writes it into the orient it asks; the window,
  handed an orient with it over the link, sets the model's lift first, as its row does.
- The report's `supports.lifted_mm` stays how high the model stands once placed.

## Consequences

- The command line again supports a model under its floor, by asking for it, and the window,
  the link and the command line weigh an orient at the same height.
- `.encrust` does not keep the asked lift: a file opened again orients at the profile's lift
  until a lift is asked again, as the window already does.
- Reopen if the asked lift is to survive a save; it then belongs in `ObjectState`.

## Alternatives considered

### Lift by the profile's `z_lift_mm` before supports, as before ADR 0472

What the command line did. It differs from the window, which lifts nothing by itself, and
ADR 0472 removed it as a difference to remove.

### The option that won, and what it costs

One flag and one optional field more; a model standing on the plate with `--supports` and no
`--lift` still gets nothing under its floor, which a user coming from the old behaviour has to
learn from `docs/cli.md`.
