# 0402. Place supports and Keep solid blockers by hand on the command line

- **Status:** Proposed
- **Date:** 2026-11-03

## Context

The window places a support where the user clicks in Supports' Edit mode, and a Keep solid
blocker where they click in Hollow; the command line could do neither, so a script, and a
language model over `encrust-mcp` (ADR 0397), could not either. Rule 9 of `architecture.md`
holds the command line to what the window does. Every point the command line takes —
`--drain-at`, `--channel`, `--label-at` — is plate millimetres as the model prints, lift
included, the frame the report locates findings in (ADR 0396).

## Decision

`--keep-solid X,Y,Z`, repeated, puts a ball blocker of `--keep-solid-radius` (the window's
default, 4 mm, now `core_volume::DEFAULT_BLOCKER_MM`) at each point among the cavity's
blockers, brought down by the supports' lift as drain points are; it needs `--hollow`.

`--support-at X,Y,Z`, repeated, adds a contact on the surface nearest each point to what
`core-engine` stands under the model: `SupportPlan::by_hand`, a `ByHand` of points and `only`,
which `--support-only-at` sets so none is placed automatically. A contact placed by hand is a
`Reason::Steady` need, built as sturdy as a support that steadies a part; it needs `--supports`
or `--support-profile`. The orientation search proves its poses without them, since they stand
where the chosen pose puts them.

## Consequences

A script or a model can place what the window's clicks place, by the points the report gives.
A point is snapped to the nearest surface, so one given off the model lands somewhere it may
not mean; the report's `supports` counts tell. A hand support's size cannot be chosen apart
from the rest. Reopen when the window lets a hand support take a size of its own.

## Alternatives considered

### A plate file listing supports and blockers

It already carries a model's settings — but a point list per model is what a flag holds as
well, and the tools over MCP pass flags.

### The option that won, and what it costs

Hand supports share the steadying shape, and every point is one more flag on a long line.
