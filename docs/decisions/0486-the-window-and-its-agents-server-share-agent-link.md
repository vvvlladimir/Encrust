# 0486. The window and its agents' server share agent-link

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

`encrust-mcp` writes the window link's lines, its agent file and the folders under the user's
cache; the window reads them. Each kept its own copy of the agent file's types, the lines'
fields and schemas, and the folder names, so a field changed on one side broke the other only
at run time. Rule 5 moves a type two crates need into a shared crate.

## Decision

`agent-link` holds what the two say to each other and where: `Show`, `PlateLine` under a
`Header`, the answers `ShowReply`, `HandedPlate` and `ApplyReply`, the window's `WindowFile`,
`AgentFile` with `Working`, and `JOBS`, `AGENTS`, `WINDOW_FILE` under `cache_dir`. It depends on
`core-edit`, `directories` and `serde` alone; the window takes it on the desktop only, as it took
`directories`. Reading a plate stays in the window, running a job in the server.

## Consequences

A line or file changed is changed once, and both sides fail to build together. The server
gains no edge to the window, nor the window to the server. Reopen if a third front end speaks
the link and needs more than these types.

## Alternatives considered

### Keep the copies and test both against one fixture

Rejected: two copies of one type is what rule 5 forbids; a fixture only finds the drift later.

### Put the types in `core-edit`

Rejected: an edit is a plate's action; the link's files and folders are not that crate's concern.

### The option that won, and what it costs

One more crate in the graph and in hakari's set.
