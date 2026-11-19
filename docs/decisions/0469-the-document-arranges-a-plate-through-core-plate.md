# 0469. The document arranges a plate through core-plate

- **Status:** Proposed
- **Date:** 2026-11-19

## Context

ADR 0466 puts every edit's work in `core-document`. `Arrange` packs each printable object's
footprint, its supports' included, over a plate and turns one the least where no slide fits.
The window did that over `core-plate`'s `arrange`, `Footprint` and `PlateArea`, with its own
outline and turn steps beside them; the command line spreads pieces in its own way. ADR 0467
names the cores the document reaches, and `core-plate` is not one of them, though
`core-engine`, which the document already depends on, builds it.

## Decision

`core-document` depends on `core-plate` directly and holds the footprint, pack and turn steps
of an arrangement, which the window calls; the window keeps only its message. The work runs
per object on `rayon`. `docs/architecture.md` draws the edge.

## Consequences

An arrangement asked by the window, by the command line's `--edits`, by a recipe's arrange step
or over the link lands in the same places. The build is unchanged, since `core-engine` built
`core-plate` for the document already. The command line's own piece spreading
(`stage/room.rs`) still turns pieces by its own outline; reopen when it runs on a `Document`.

## Alternatives considered

### Reach core-plate through core-engine's re-exports

No new edge in the manifest, but `core-engine` would re-export a plate API it does not use, an
edge hidden instead of drawn.

### The option that won, and what it costs

`core-document` now names one more core, and a change to `core-plate`'s arrange reaches the
window and every edit at once.
