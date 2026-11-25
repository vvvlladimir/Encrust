# 0465. The window lists its agents and shows their work when asked

- **Status:** Proposed; what the ring lists narrowed by 0476, a plate per job by 0477
- **Date:** 2026-11-18

## Context

ADR 0459 has `encrust-mcp` hand each revision it keeps to the open window, which runs it on
the plate at once. A window opened after the work sees nothing: the server hands over only
the moment it keeps a revision, and the window reads no job from disk. A window already open
is taken over: each revision moves its printer and resin, the window's for every plate, and
starts Prepare for print on the plate in hand while the user works on it. Several chat
clients can drive their own servers at once, each with its own jobs, and the window cannot
tell them apart. Every job is kept on disk under the user's cache (ADR 0455); the window
already finds the server's folder there.

## Decision

- **Each server is an agent with a file of its own** in `agents/` beside `mcp-jobs/`: its
  process, the client's name as the client introduced itself, when it started and was last
  seen, what tool it runs on which job, the jobs it touched, latest first, and when it
  ended. It is written whole on each call and every 20 s; one silent for a minute is read
  as ended, and one gone a week is taken away. `--agents DIR` moves it; `--no-window`
  keeps none.
- **The window lists them** from a ring left of the print chip, beside the printers', there
  while an agent works or waits for its chat: a robot glyph, an arc going round while one
  works — no share, since neither a chat nor its tools say how much is left — and a count
  past one; on hover each agent's model, what it does and for how long. It opens a chooser
  of the agents, each under its name with its latest jobs as rows — model, revision, the check's
  verdict — and a press on a row shows that revision on the plate as ADR 0459 runs one. A
  project's job opens the project. The jobs of the last day no agent's file names follow
  as *Earlier jobs*, so the work of a server that keeps no file is listed all the same.
- **Following is the user's choice:** with *Follow agents* off, the default, a revision
  handed over is answered `shown: false` and only listed; on, it runs as ADR 0459 has it.
  The switch is in the chooser and remembered with the window's behaviour.
- **`show_in_window`** of `encrust-mcp` hands a revision over marked `open`, which the
  window shows whether it follows or not; with no window answering, it starts the window —
  `--app PATH`, else `encrust-gui` beside the server or on the `PATH`, else the installed
  app — and hands it over once the window's file answers, within 30 s.

## Consequences

A window opened after the work lists it and shows it with one press, and a window in use is
left alone unless the user asked to follow or the client to show. Several agents are told
apart by client and start, each with its jobs. The window now reads the server's job layout
(`N.args.json`, `N.recipe.toml`, `N.json`) from disk as well as the line it is sent, so the two
change together. The ring is gone when no agent is there, and the browser build has none. The
agents' part is where a chat of the window's own goes later (step 9). Reopen if the window
ever runs agents itself rather than listing those the clients run.

## Alternatives considered

### Keep following by default, and queue what comes while no window is open

Nothing new to choose — but a window in use is still taken over, which is what the user
asked to stop, and a queue is a second store of what the job folders hold already.

### Each agent's work on a plate of its own

Leaves the plate in hand alone — but plates belong to the project the user is working on;
an agent's model is not part of it until the user shows it there.

### The option that won, and what it costs

A file per server written on every call, a scan of a small folder every two seconds off the
drawing thread, and one more place the job layout is read.
