# 0477. An agent's job stands on a plate of its own in the window

- **Status:** Proposed
- **Date:** 2026-11-25

## Context

ADR 0466 has the open window own the plate: a server asks for it, builds its edits on it and
hands them back; ADR 0473 stands a revision shown by its plate on the active plate, beside
what stands, in place of the objects its last revision shown stood as. Two things went wrong
in use. The plate handed over held the user's own models, so an agent cut, arranged and
checked a model it was never given and reported it as foreign. The window noted a job's
objects only when it showed a revision by its plate; after edits replaced them — a cut's two
halves — it still held the cut model's id, so the next revision shown by its plate stood
beside the halves instead of in their place. The user expects a followed window to show what
the agent sees, no more.

## Decision

- **A job's revisions stand on a plate of their own,** added and named for the job's model
  the first time one is shown, numbered past a name taken, and shown while it lands. A
  revision shown by its plate takes the place of everything on that plate.
- **The server is handed that plate alone** (`want: plate`), as a project of one plate; a
  job with none yet is handed an empty one, so it builds by its flags and is shown.
- **Edits land on that plate,** shown as each starts, so an arrange spreads the job's
  models and no others.
- A plate the user renames or removes is no longer the job's: its next revision gets a new
  one. The rest of ADR 0466 — versions, `stale`, edits as values — stands; this replaces
  ADR 0473's "beside what stands" and lifts ADR 0465's rejection of a plate per job.

## Consequences

The window shows a job as the agent checks it, and a user's models are neither seen nor
moved by an agent. What the user changes on the job's plate still reaches the agent's next
step. A project saved while following holds the jobs' plates as plates of its own. Following
switches the shown plate whenever a revision lands. Reopen if users want an agent to work
among their own models on one plate.

## Alternatives considered

### Track the job's objects through its edits, on the shared plate

Fixes the halves left behind — but the agent still builds on the user's models and arranges
them with its own, which is what the user asked to stop.

### Each revision replaces the whole active plate

Mirrors the agent with no new plate — but takes the user's models off the plate in hand.

### The option that won, and what it costs

A plate per followed model in the user's project, which they remove by hand when done, and
a shown plate that changes under the user while an agent works.
