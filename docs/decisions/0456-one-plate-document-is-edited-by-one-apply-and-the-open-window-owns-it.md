# 0456. One plate document is edited by one apply, and the open window owns it

- **Status:** Accepted
- **Date:** 2026-11-18

## Context

Each action on a plate was written three times — a flag of `encrust-cli`, a step of each recipe
executor, a tool of the window — and the copies differed: the window's cut keeps drain holes
and lays halves on their cut faces, the command line's did neither; supports were grown by the
window's job with paint and seeds and by `core_engine::stand_under` without; arrange packed
differently. A cut an agent made never reached the window, and an undo there never reached the
agent. `.encrust` (ADR 0191) holds every object's state, but no object had an id that outlives a
reorder.

## Decision

- **An edit is a value.** `core-edit` holds `Edit`, one variant per action, naming its object by
  `ObjectId` and its points in plate mm, its settings the types `.encrust` stores. Print
  settings, printer and resin are no edit.
- **One apply.** `core-document` holds `Document`, the plate as `.encrust` stores it, and
  `apply(&mut Document, &Edit, Run)`. The window, the command line and the server edit through
  it; where the front ends differed, the window's behaviour wins. It depends on `core-engine`,
  `core-text`, `core-mesh-io` (one reader of a picture into heights) and `core-plate`
  (footprints, pack and turn, per object on `rayon`); `core-edit` on `core-volume` for
  `RepairMethod`.
- **Supports are grown the window's way.** The window's steps — needs from the slices, fed the
  painted, blocked and shown faces and seeds, then strengthened — move into `core-engine`;
  `stand_under` and the autopilot call them, so every front end grows the same trees.
- **Recipes yield edits.** `core-recipe` answers a plate step with its edits (`edits_of`);
  `Edit::Orient` stands a model by the search with no machine's trial. Check, save, gate and
  pause are no edit.
- **Ids are stored**: `ObjectState` gains `id`, assigned on reading an older file, so the
  manifest stays at version 7.
- **The open window owns the plate.** It counts a `version` on every edit, undo and redo. The
  server asks for the plate, builds on it and hands back the edits with the version they were
  built on; a window that moved on answers `stale`, and the server rebuilds once.

## Consequences

What an agent does lands in the window as it would by hand, and an undo there is what its next
call sees. A new action is a variant and a case of `apply`, then a flag, a panel and a tool. The
command line's supports, cut and arrange change to the window's; `support-lab`'s baselines are
run again. `encrust-mcp` builds the cores `core-edit` names. The window's undo stays a snapshot
(ADR 0086). Reopen if a front end needs an edit `apply` cannot express without its own state.

## Alternatives considered

### Add the missing steps to `core-recipe`

Shortest to the window showing a cut, but every action stays written three times and an undo
never reaches the agent.

### Plain-data `core-edit` with copies of the settings types

Keeps the server off the cores, at the price of every settings type written twice.

### The engine's support path for every front end

What the autopilot was tuned on — but the window would lose its paint, seeds and grown trees.

### The option that won, and what it costs

Thousands of lines move across the window, the command line and two new crates; the server's
build takes the cores; a change to the picture reader or to arrange reaches every front end.
