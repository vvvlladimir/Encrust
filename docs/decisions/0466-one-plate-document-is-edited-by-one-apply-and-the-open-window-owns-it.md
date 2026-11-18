# 0466. One plate document is edited by one apply, and the open window owns it

- **Status:** Proposed
- **Date:** 2026-11-18

## Context

A cut an agent makes through `encrust-mcp` never reaches the window. ADR 0459 hands the window
a revision's `core-recipe` file, and no recipe step says cut, scale, label, points, a pose
picked and more (`encrust-mcp/src/recipe/unshown.rs`). Each action is written three times —
a flag of `encrust-cli`, a step of each recipe executor, a tool of the window — and the window
and the command line already differ: the window's cut keeps the model's drain holes and lays
the halves on their cut faces, the command line's does neither. The link runs one way: an
undo in the window never reaches the agent, whose next call is built on its own last revision
and brings the cut back. `.encrust` (ADR 0191) already holds every object with its transform,
cavity, supports and painted faces, but no object has an id that outlives a reorder.

## Decision

- **An edit is a value.** `core-edit` holds `Edit`, one variant per action a user or an agent
  takes on a plate, each naming its object by `ObjectId` and its points in plate millimetres.
  Its settings are the types `.encrust` stores already (`Axis`, `Keep`, `Joints`, `CutState`,
  `HollowState`, `DrainState`, …), so `core-edit` depends on `core-engine` and `core-joint`
  and copies no type. Print settings, printer and resin are not edits: a revision names them.
- **One apply.** `core-document` holds `Document`, the plate as `.encrust` stores it with
  stable ids, and `apply(&mut Document, &Edit, Run) -> Result<Applied, ApplyError>`, `Run`
  carrying the cancel flag and progress every window job already takes. The window, the
  command line and the server's checks all edit through it; the code moves down from
  `encrust-cli` and the window's `job/*`, and where the two differed the window's behaviour
  wins.
- **Ids are stored.** `ObjectState` gains `id`, absent in older files and assigned on reading,
  so the manifest's version stays 7.
- **The open window owns the plate.** It counts a `version`, raised on every recorded edit,
  undo and redo. Before each edit the server asks it for the plate (`want: plate`) and builds
  on that `.encrust`; it hands the window the edits with the version they were built on, and
  a window that has moved on answers `stale`: the server fetches again and rebuilds once, then
  tells the client. With no window answering, the job's last `.encrust` is the base. Link
  schema 3, `docs/design/window-link.md`.
- **A revision is a base and its edits:** `N.edits.json` and the `N.encrust` its check wrote.
  `encrust estimate` takes `--edits FILE` and `--project-out FILE`, and accepts an `.encrust`
  with edits. The recipe and the flags a revision was turned into go.
- **Recipes yield edits**: `core-recipe` decides a step and names the `Edit`s it makes; both
  recipe executors go.

This supersedes 0459 and continues 0455 by its first workspace edge, `encrust-mcp ──> core-edit`.

## Consequences

What an agent does lands in the window as it would by hand, and an undo there is what the
agent's next call sees. An action is written once: a new one is a variant and a case of
`apply`, then a flag, a panel and a tool that build it. The command line's cut changes to the
window's. `encrust-mcp` builds the cores it names through `core-edit`, which it did not; its
process boundary to `encrust` stays. The window's undo stays a snapshot (ADR 0086). The move is
long: until every action is an `Edit`, the old path runs the rest, and `unshown.rs` lists what
is left. Reopen if a front end needs an edit `apply` cannot express without its own state.

## Alternatives considered

### Add the missing steps to `core-recipe`

Shortest to the window showing a cut, but the action is still written three times, and an undo
still never reaches the agent.

### Plain-data `core-edit` with copies of the settings types

Keeps `encrust-mcp` free of the cores, at the price of every settings type written twice and
converted between the copies.

### Settings types moved down into `core-edit`

No copies and a light server, but `Joints` and `HollowSettings` carry `core-geometry` types,
so the server is not light after all, and four cores turn to depend on `core-edit`.

### The option that won, and what it costs

Thousands of lines move across the window, the command line and two new crates over many
commits, and the server's build takes the cores.
