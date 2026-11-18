# The window link

How `encrust-mcp` hands each revision it keeps to an open window, so a chat client's steps
land in the viewport and on the undo stack (ADR 0459; the plan is `ai-integration.md`).

## Finding the window

A window on the desktop listens on a free port of `127.0.0.1` and writes `window.json` in the
user's cache directory — `ProjectDirs("", "", "Encrust").cache_dir()`, beside the server's
`mcp-jobs` — readable by the user alone:

```json
{"schema": 2, "port": 50123, "token": "9f0c…32 hex digits", "pid": 4242}
```

A window that closes takes the file away unless another has written over it; the last window
opened is the one shown to. The server reads the file before each revision it hands over: no
file, or a port nobody answers on, is no window, and the tool answers as it did alone.
`--window FILE` reads another file, `--no-window` none.

## One revision

Each check the server runs passes `--recipe-out`, and the recipe written is kept as
`N.recipe.toml` beside the revision's `N.toml`. After it keeps a revision, the server
connects, writes one line of JSON and reads one back:

```json
{"schema": 2, "token": "…", "job": "cube-18e4…", "revision": 2, "model": "/m/cube.stl",
 "printer": "elegoo-mars-4-ultra", "resin": "my-grey", "recipe": "name = \"Prepare for print\"\n…"}
```

`printer` and `resin` are catalogue ids or TOML paths, as the job holds them. A line with
another token or schema is refused and never reaches the plate. The window answers on its
next frame, at most 10 s later:

```json
{"shown": true}
{"shown": false, "why_not": "the revision's recipe: …"}
```

The tool's answer carries it as `window`, and beside it `unshown`: what the job asks that no
recipe step says, which the window leaves out.

## What the window does with it

The first revision of a job reads its model in, as Open does; a model taken off the plate is
read in again for the job's next revision. Each revision then puts the model back as it came
in, takes away what the last run left beside it — plugs, say — puts the window on the job's
printer and resin, and starts Prepare for print by the revision's recipe on that model. The
run is one undo entry. A newer revision of the job stops the run of an older one; another
job's run, or a job of the user's own still landing, holds it until it ends.

The check every tool answers with is still the command line's: the window shows the steps, it
does not answer for them.
