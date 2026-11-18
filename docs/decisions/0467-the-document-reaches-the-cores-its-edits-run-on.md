# 0467. The document reaches the cores its edits run on

- **Status:** Proposed
- **Date:** 2026-11-18

## Context

ADR 0466 puts every edit's work in `core-document`, moved down from the window and the command
line. Some edits run on a core neither front end shares with `core-engine`: a label is laid by
`core-text`, a picture pressed in as relief is read by `core-mesh-io`, the one reader of a
picture in the workspace. `core-edit` names a repair by `RepairMethod`, a `core-volume` type.
Each edge would otherwise be a type copied or a computation left in the front ends.

## Decision

`core-document` depends on `core-text` and `core-mesh-io`, and `core-edit` on `core-volume`,
each for what its edits run on or name. `core-document` reads a picture through
`core-mesh-io`'s `Texture` and one bytes-to-heightmap reader, which the window calls as well.
No core reaches a front end; `docs/architecture.md` draws the edges.

## Consequences

Every edit's work has one home, and a picture reads to the same heights in the window, on the
command line and over the link. `encrust-mcp`, which reaches `core-edit`, builds `core-volume`
too, as ADR 0466 already accepted for the cores. Reopen if `core-document` comes to need a
container's reader or anything of the window's.

## Alternatives considered

### The caller reads the picture and the edit carries the heights

Keeps `core-document` off `core-mesh-io`, but an edit then carries a picture's pixels instead
of its path, and every front end reads and caps it in its own way.

### The option that won, and what it costs

`core-document` builds two more cores, and a change to the picture reader rebuilds it.
