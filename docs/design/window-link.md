# The window link

How `encrust-mcp` hands each revision it keeps to an open window, so a chat client's steps
land in the viewport and on the undo stack (ADR 0401; the plan is `ai-integration.md`).

## Finding the window

A window on the desktop listens on a free port of `127.0.0.1` and writes
`window.json` in the user's cache directory — `ProjectDirs("", "", "Encrust").cache_dir()`,
beside the server's `mcp-jobs` — readable by the user alone:

```json
{"schema": 1, "port": 50123, "token": "9f0c…32 hex digits", "pid": 4242}
```

A window that closes takes the file away unless another has written over it; the last window
opened is the one shown to. The server reads the file before each revision it hands over: no
file, or a port nobody answers on, is no window, and the tool answers as it did alone.
`--window FILE` reads another file, `--no-window` none.

## One revision

The server connects, writes one line of JSON and reads one back; each connection is one
revision. The line is the revision as the job keeps it, its recipe in the TOML's shape
(`docs/cli.md`, `encrust-mcp`):

```json
{"schema": 1, "token": "…", "job": "cube-18e4…", "revision": 2,
 "recipe": {"model": "/m/cube.stl", "printer": "elegoo-mars-4-ultra", "resin": "my-grey",
            "prepare": true, "supports": {"profile": "auto", "cavity": true}}}
```

A line with another token or schema is refused and never reaches the plate. The window
answers on its next frame, at most 10 s later:

```json
{"shown": true, "unshown": ["the light profile: supports are sized by the automatic one"]}
{"shown": false, "why_not": "the token is not this window's"}
```

The tool's answer carries it as `window`. A field the window does not read is passed over,
so the server may add to the recipe without breaking an older window.

## What the window does with it

The first revision of a job reads its model in, as Open does. Each revision then puts the
model back as it came in and replays the recipe on it as a Prepare for print run with a plan
(`prepare/plan.rs`): a step the recipe sets is done as it says, one it leaves out is decided as
the autopilot decides it under `prepare` and skipped otherwise. The run is one undo entry; a
newer revision of the job stops the replay of an older one, and a job of the user's own
running on the plate holds the replay until it ends. A model taken off the plate is read in
again for its job's next revision.

Replayed as the recipe says: the orientation search and its goal, the repair mode, the wall,
automatic drains, automatic supports and the cavity's. Named in `unshown` and left out: a turn
given in degrees, `max_tilt_deg`, an open floor and infill, drain points and plugs, a named
support profile, and the printer and resin, which stay the window's own.

The check every tool answers with is still the command line's: the window shows the steps,
it does not answer for them.
