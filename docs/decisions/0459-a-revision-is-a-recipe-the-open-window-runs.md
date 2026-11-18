# 0459. A job's revision is a recipe, which the open window runs

- **Status:** Proposed
- **Date:** 2026-11-18

## Context

A chat client driving `encrust-mcp` (ADR 0455) leaves its work in a job directory, each
revision a TOML of the server's own that becomes `encrust` flags. A user with the window open
sees none of it until a file is written. The window prepares a model by a `core-recipe` file
(ADR 0403), the steps Prepare for print takes, one undo entry a run. The command line runs the
same files with `--recipe` and `--prepare`, a flag typed for a model winning over the setting
it shapes and forcing its step to run. `encrust-mcp` depends on nothing in this workspace,
so it cannot write a recipe itself. The server runs as a child of the client, the window as
the user's own program: they meet only through something local.

## Decision

**A revision is the recipe its check ran.** `estimate --recipe-out FILE` writes the recipe a
run takes each model through, every flag typed written into the steps it shapes: a forced
step loses its `when`, a typed setting replaces the step's own and every choice's; with no
recipe, the steps the flags ask for alone, in the command line's order. The server passes it
on every check and keeps the file as `N.recipe.toml` beside the revision. What no step says
yet — the modelling, slicing, a pose picked or turned, points of any kind — stays in the
server's TOML as flags, and the tool's answer lists it as `unshown`.

**The window listens on a loopback TCP port** with a thread of its own, and writes the port
and a 128-bit token into `window.json` in the user's cache directory, readable by the user
alone, taken away when it closes. After every revision it keeps, the server sends the job,
the revision's number, its model, printer and resin and the recipe's text as one line of
JSON with the token, and puts the window's answer in the tool's answer as `window`.
`--window FILE` and `--no-window` steer it.

**The window runs the recipe** as a Prepare for print run on the job's model, read in on the
first revision and put back as it came in on each later one, on the job's printer and resin,
what the last run left beside it taken away; one undo entry a revision. A newer revision of a
job stops the run of an older one; another job's run is waited for.

## Consequences

The window and the command line run one file, so what the window shows is what the check
checked, for every step a recipe holds; each setting a recipe step gains is one more thing the
window runs, with nothing in the link to change. Each revision is staged twice, by the
command line for the check and by the window for the plate. A recipe of the autopilot keeps
its conditions, so the window decides them on its own plate, as the command line did on its
own. Any program of the user's that reads their cache can drive the window; nothing from the
network can. Reopen when the window answers the checks itself (step 9), or when `unshown`
empties.

## Alternatives considered

### The window reads the server's TOML

The branch did this first: a plan of the window's own, built from the server's fields, and
a translation of each into a tool's settings — about 3,600 lines, and a third way of saying
what a step does, which main's recipes made a second copy of.

### `encrust-mcp` writes the recipe

It would need `core-recipe`, its first edge into the workspace, or a copy of the recipe's
format and of the autopilot's steps.

### A Unix socket or a named pipe

The socket file's permissions would guard it with no token — but Windows needs a named pipe,
code of each system's own, for the guard a token in a user-only file gives.

### The option that won, and what it costs

Until recipe steps take points, a picked pose and the modelling, the window shows less than
the command line checks, and says so in `unshown`; every check runs the command line once more
to write the file.
