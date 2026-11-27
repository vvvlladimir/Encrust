# 0462. The window lists its agents and shows each job on a plate of its own

- **Status:** Accepted
- **Date:** 2026-11-18

## Context

A chat client's server runs as the client's child, the window as the user's program: they meet
only through something local. A user with the window open saw nothing of a job until a file was
written; several clients may run servers at once. A revision replayed in the window as a recipe
left out what no step says — points, a picked pose, the modelling. The plate handed to a server
held the user's own models, so an agent cut and arranged models it was never given. Edits that
landed in part left the window holding half a revision the job kept whole.

## Decision

- **The link:** the window listens on a loopback port and writes it with a 128-bit token in
  `window.json` in the user's cache, readable by the user alone. `docs/design/window-link.md`.
- **A revision is shown by its plate**: each check passes `--project-out`, kept as `N.encrust`,
  and the window reads it off the drawing thread. Each connection runs on a thread of its own,
  at most eight.
- **Each job has a plate of its own**, named for its model; the server is handed that plate
  alone, edits land on it, and a revision's plate replaces it. A plate the user renames or
  removes is no longer the job's.
- **Edits land whole or not at all**: held as one undo entry; one that fails puts the plate back
  and the server keeps no revision built of them.
- **The agents' ring**: each server writes its file in `agents/`; the window lists a live agent
  only while it has a model in hand, a press shows that job. A row is hidden for the session or
  its job folder deleted; the window never stops an agent's tool.
- **Following is the user's choice**, off by default; `show_in_window` shows a revision whether
  or not, starting the window if none answers.

## Consequences

The window shows what the command line checked, every step and point of it, and a user's models
are neither seen nor moved by an agent; an undo is what the agent's next step sees. A plate per
followed job stays in the project until the user removes it. A client may lose a step the window
could not land, and the answer says why. Any program that reads the user's cache can drive the
window; nothing from the network can. `encrust-cli` has no ring: it is the window's view of the
agents' files. Reopen if users want an agent among their own models on one plate.

## Alternatives considered

### Replay the revision's recipe in the window

No file a check, but a second stager whose plate differs, and what no step says never shows.

### A Unix socket or a named pipe

The socket's permissions would need no token, but Windows needs code of its own.

### Each revision replaces the whole active plate

Mirrors the agent with no new plate, but takes the user's models off the plate in hand.

### The option that won, and what it costs

A plate file written each check, a thread per line, an undo entry held across frames, a file
per server written each call and a folder scanned every two seconds.
