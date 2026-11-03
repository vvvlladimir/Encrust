# 0401. The open window replays each revision the server keeps

- **Status:** Proposed
- **Date:** 2026-11-03

## Context

A chat client driving `encrust-mcp` (ADR 0397) leaves its work in a job directory; a user with
the window open sees none of it until they open the file written at the end. The plan wants
each step in the viewport and on the undo stack as the client makes it. The server runs on the
user's computer as a child of the client, the window as the user's own program, so they meet
only through something local: a socket the window listens on, found through a file. The
window already chains its tools' jobs into one Prepare for print run, one undo entry, from the
decisions `--prepare` makes (ADR 0399). `encrust-mcp` depends on nothing in this workspace,
and the window cannot depend on a binary.

## Decision

**The window listens on a loopback TCP port** with a thread of its own, and writes the port
and a 128-bit token into `window.json` in the user's cache directory, readable by the user
alone, taken away when it closes. After every revision it keeps, the server connects, sends
the revision — job, number and recipe — as one line of JSON with the token, and puts the
window's one-line answer in the tool's answer as `window`. No file, or nobody on the port, is
no window. `--window FILE` and `--no-window` steer it.

**The window replays the revision** on the plate: the job's model read in on its first
revision and put back as it came in on each later one, then a Prepare run by a **plan** that
takes each step the recipe sets as given, decides what it leaves out as the autopilot would
under `prepare` and skips it otherwise. What it cannot replay yet it names in `unshown`. The
command line still answers every check. Each side reads the line into types of its own, as
the server reads the command line's report: the schema is the contract, written in
`docs/design/window-link.md`. The window takes `directories`, already the catalogue's, to find
the cache.

## Consequences

Every step a client takes shows in the viewport and undoes as one entry, with no change to
the client or its tools, and the server works as before when no window is open. Each revision
is staged twice, by the command line for the check and by the window for the plate, and is
replayed from the model as it came in, so a long hollow pays again in the window. A turn given
outright, infill, drain points, plugs and named support profiles are not replayed yet. Any
program of the user's that reads their cache can drive the window; nothing from the network
can. Reopen when the window answers checks itself from the plate in memory (step 9's catalogue
moving down), or when a client needs the window's own state back.

## Alternatives considered

### A Unix socket or a named pipe

The socket file's permissions would guard it with no token — but Windows needs a named pipe,
which is a new dependency or code of each system's own, for the same guard a token in a
user-only file gives.

### The server sends the command line's flags, or a crate of the recipe shared by both

The window cannot read `encrust` flags without the command line's parser, and a shared crate
would give `encrust-mcp` its first edge into the workspace and the window a second definition
of the tools' schema to build. A line of JSON read on each side keeps both as they are.

### The window answers the check itself

No stage paid twice, and the check is the plate the user sees — but the window builds no
report of the command line's shape, and that is the catalogue moving into the window, step 9.

### The option that won, and what it costs

Two copies of the recipe's shape, held together by a schema number and a page, and every
revision cut twice.
