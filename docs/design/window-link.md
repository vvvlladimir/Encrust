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
newer revision of the job stops the replay of an older one, while another job's replay, or a
job of the user's own running on the plate, holds the replay until it ends. A model taken off the plate is read in
again for its job's next revision.

Replayed as the recipe says: the orientation search with its goal and tilt limit, the way it
ranks that `pick` names and the points kept clear in the turn they were seen in, or a turn
given in degrees, turned as `--rotate` turns; the repair mode; the wall with its open floor and
infill, measured inside at the command line's precision; holes drilled into the surface nearest
each point, at the recipe's diameter, then the automatic drains with or without vents; supports
by the automatic profile or the one named, kept as the model's own for the group in hand, and
the cavity's, with those placed by hand at their points; the scale, before anything else; the
hollow's mode, precision and Keep solid balls; a hole's depth, taper, sides, turn and plug gap.
After the repair, in the command line's order, each by the window's own tool set as the
recipe says: faces turned, bridged and dropped and shells dropped, by the numbers `inspect`
gives; the relief, its picture read where one is named; combine, the other models read in as
the job's own; the label, at the surface nearest its point; the cut with its joint; the split,
or the part cut off at a point; the plate arranged. A step that makes pieces — a cut, a split,
a combine — hands them to the steps after it, and the next revision takes them away before it
puts the models back. Walled holes are drilled before the cavity, channels dug through the
points given, holes into the supports drilled once the trees are built.

The window is put on the recipe's printer and resin, catalogue ids or TOML files, and takes
its slicing: layer height, adaptive layers, exposure bands, anti-aliasing, greys, blur and
islands removed; a revision that gives no layer height leaves the window's as it is. The
window lays a plug beside every hole whatever the recipe says, and arranges every model on
its plate where the command line arranges the job's; `unshown` says both.

The check every tool answers with is still the command line's: the window shows the steps,
it does not answer for them.
