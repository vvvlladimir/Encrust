# 0480. A job is hidden or deleted from the agents' list

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

The agents' ring lists each live agent with the job in its hand (ADR 0476). A job stays
there as long as its agent waits for its chat, finished or not, and its folder under
`mcp-jobs/` stays on disk after the agent ends. Users asked to take a job off the list and to
delete one. The window only reads the agents' files; it has no way to tell a server to stop a
tool, and a stop that ends the server would leave its chat client without its tools.

## Decision

- **Hide:** a job's row has a quiet close button. It takes that job off the list for as
  long as the window runs. The same agent's next job is listed again. Nothing on disk changes.
- **Delete:** a quiet bin, which asks by a second press, takes the row off and removes
  `mcp-jobs/<id>/` with every revision in it, on a thread of its own. It waits while the
  agent runs a tool on that job. Only a plain job name is deleted, never a path.
- **No stop:** the window does not interrupt an agent's tool. The chat client's own cancel
  stops a run, as before.

## Consequences

The list can be cleared of finished work without ending a chat. A deleted job cannot be
shown again, and an agent that later names it is told by the server that the job has no
revision. Hidden jobs are forgotten when the window closes. `encrust-cli` has no
counterpart: the list is the window's view of the agents' files, and no core call is made. Reopen if users ask the window to stop an agent:
that needs a channel from the window to the server.

## Alternatives considered

### A stop file the server watches

The window would write a file the server polls during a call, then kill the `encrust` run.
Left out for now: it adds a protocol between two crates for a case the chat client already
covers.

### The option that won, and what it costs

A hidden job is not remembered across sessions. An agent working on a job keeps its row,
whose bin waits until the tool ends.
