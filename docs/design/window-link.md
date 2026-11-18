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

What follows is what the window does with a revision it shows (see Following). The first revision of a job reads its model in, as Open does, unless another job of the
same model file is followed: the new job takes that one's place, so a model stands on the
plate once however many jobs an agent starts on it; a model taken off the plate is
read in again for the job's next revision. Each revision then puts the model back as it came
in, takes away what the last run left beside it — plugs, say — puts the window on the job's
printer and resin, and starts Prepare for print by the revision's recipe on that model. The
run is one undo entry. A newer revision of the job stops the run of an older one; another
job's run, or a job of the user's own still landing, holds it until it ends.

The check every tool answers with is still the command line's: the window shows the steps, it
does not answer for them.

## Agents

Each server keeps a file of its own in `agents/` beside `mcp-jobs/`, named for its process
and start, written whole — into a `.part` file, then renamed — on each tool call and every
20 s (ADR 0465):

```json
{"schema": 1, "pid": 4242, "client": "claude-code", "started_s": 1763460000,
 "seen_s": 1763460125, "working": {"tool": "set_hollow", "job": "cube-18e4…",
 "since_s": 1763460120}, "jobs": ["cube-18e4…", "bust-18e3…"]}
```

`client` is the name the client gave itself on connecting; `working` is there only while a
call runs; `jobs` are the jobs its calls named or answered with, latest first, at most 20.
A server that ends writes `ended_s`; one whose `seen_s` is a minute old ended without
saying. A file whose `seen_s` is a week old is taken away by the next server that starts.

The window reads the folder every 2 s off its drawing thread and lists the agents in the
agents' ring beside the print chip (`ui-screens.md`, Top bar): each job an agent's file names, by its
latest revision — the highest `N.toml` — whose model is the first of `N.args.json` and whose
verdict is `N.json`'s `status`. The jobs of the last day — their folder changed within it — that no agent's file names are
listed after the agents as *Earlier jobs*: a server that keeps no file, as one built before
ADR 0465, still has its work listed. A press on a job builds the same line the server sends from
`N.args.json` (`--printer` or `--profile`, `--resin` or `--material`) and `N.recipe.toml`,
marked `open`, and takes it as one handed over.

## Following

A line marked `"open": true` is shown whatever the window does. One not marked is shown only
while *Follow agents* is on; otherwise the window answers
`{"shown": false, "why_not": "the window is not following its agents; …"}` and the revision
waits in the list.
