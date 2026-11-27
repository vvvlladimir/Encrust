# The window link

How `encrust-mcp` hands each revision it keeps to an open window, so a chat client's steps
land in the viewport and on the undo stack (ADR 0462; the plan is `ai-integration.md`).

## Finding the window

A window on the desktop listens on a free port of `127.0.0.1` and writes `window.json` in the
user's cache directory — `ProjectDirs("", "", "Encrust").cache_dir()`, beside the server's
`mcp-jobs` — readable by the user alone. Both sides take the lines, the files and these names
from `agent-link` (ADR 0455):

```json
{"schema": 4, "port": 50123, "token": "9f0c…32 hex digits", "pid": 4242, "version": "0.9.0"}
```

`version` is the window's; where it is not the server's own, the server's next tool answer
carries one note telling the user to install the extension from the window (ADR 0455).

A window that closes takes the file away unless another has written over it; the last window
opened is the one shown to. The server reads the file before each revision it hands over: no
file, or a port nobody answers on, is no window, and the tool answers as it did alone.
`--window FILE` reads another file, `--no-window` none.

## One revision

Each check of a model the server runs passes `--project-out`, and the plate written is kept
as `N.encrust` beside the revision's `N.toml` (ADR 0462); `--recipe-out` keeps the recipe it
ran as `N.recipe.toml` for `save_recipe`. After it keeps a revision, the server connects,
writes one line of JSON and reads one back:

```json
{"schema": 4, "token": "…", "job": "cube-18e4…", "revision": 2, "model": "/m/cube.stl",
 "printer": "elegoo-mars-4-ultra", "resin": "my-grey", "plate": "…/cube-18e4…/2.encrust"}
```

`printer` and `resin` are catalogue ids or TOML paths, as the job holds them. Each connection
is answered on a thread of its own, at most eight at once, so a line waiting on the window holds
up no other (ADR 0462). A line with another token or schema, or a plate that is not there, is
refused and never reaches the plate. The window takes the revision on its next frame and
answers then. A tool's answer waits 2 s for it, `show_in_window` 15 s; past that the answer says
`"shown": "pending"` and the window is handed the revision meanwhile, the server's next line
to it waiting until it is taken, so lines arrive in the order sent; the plate is read on a thread aside and stood
once read, a plate that does not read told on the status line. The tool's answer carries the
window's as `window`:

```json
{"shown": true}
{"shown": false, "why_not": "the revision's plate …: …"}
```

A revision made by edits on the window's plate (below) is not handed over again: that plate
holds it.

## The plate and its edits (schema 3)

The open window owns the plate (ADR 0456), each job's a plate of its own (ADR 0462);
`schema` in its file is the highest line it reads. A server asks for its job's plate as it
stands:

```json
{"schema": 3, "token": "…", "job": "cube-18e4…", "want": "plate"}
{"plate": "…/mcp-jobs/cube-18e4…/base-12.encrust", "version": 12, "objects": [3, 4],
 "job_object": 3}
```

The window writes the job's plate, as a project of that one plate — an empty one for a job
with none yet — into the job's folder off its drawing thread, every object under the id it
knows it by, and answers with the path, the ids on the plate, `job_object` — the job's model
where it stands whole there, its one object that is no plug — and its version: a count raised by
every recorded change of the plate, its undo and its redo, not by a tool's setting. The server
builds its edits on that file and hands them back with the version:

```json
{"schema": 3, "token": "…", "job": "cube-18e4…", "base_version": 12,
 "apply": [{"edit": "cut", "object": 3, "axis": "z", "at_mm": 50.0}]}
{"applied": true}
{"applied": false, "stale": true, "version": 13}
{"applied": false, "why_not": "…"}
```

Edits on a plate still at `base_version` run one after another on the job's plate, shown as
each starts, as the tools' own jobs, and are answered once all have landed, within 10 min (the
server waits 11): all of them one undo entry. One that does not land stops the rest, the plate
is put back as the edits found it, and the answer says why (ADR 0462). The server then keeps no
revision of them; a revision checked by its flags before its edits were handed over is shown
by its plate instead, the answer's `then_shown`. A plate the user changes while edits land is
left as they left it, one undo entry, and answered with why. A plate that moved on is answered
`stale` with its version, and nothing is applied: the server asks for the plate again, builds
and checks the edits on it once more and keeps only that revision. A window not following its
agents refuses both lines, and edits while others are still being applied are refused.

With the window followed, a revision whose every change is an edit is built on the window's
plate: the server asks for it, checks it with the edits (`estimate PLATE --edits --project-out`)
and keeps the plate they leave as `N.encrust` and the edits as `N.edits.json`, then hands the
edits over on that plate's version. What the user changed or undid there is in the next check,
whatever the change. With no window answering, the same revision is built on the plate the
last one kept, while the job's model on it is one object; a revision made so is written and
drawn from that plate.

A revision after the first hands over, as edits of the job's model, what it asks beyond the
revision before: a repair, the faces mended, a picture pressed in, a scale, a turn typed, a
label, a split or part cut off, a cut, walled holes, a hollow with its walls kept solid, drains
— at points or by the engine's search, channels, plugs — supports — by a preset or sized
automatically, under a cavity, off what shows — points placed by hand, merges and holes into
the raft, then, where it cuts pieces with `arrange` or asks `arrange` anew, an `Arrange` over
the usable area the last check reported, in that order; with no area known it goes by its
flags. One that asks anything no edit carries yet — a combination — or takes
back what an earlier one asked is checked by its flags and shown by its plate instead, as
below. A pose the search picks is handed over as an `Orient`
edit on the job's printer, its places kept clear with it; on a printer of a file, as the turn the
check chose (`oriented.rotate_deg`). While the window holds a cut it was
handed, the job's model is no longer on its plate whole, and a revision keeping that cut is not
shown; a cut the user undid there is taken out of the job's next revision.

## What the window does with it

What follows is what the window does with a revision it shows (see Following). Once no job
runs on its plate — a tool's, a Prepare for print, an import — it puts the window on the
job's printer and resin, shows the job's plate — a new one, named for the model and numbered
past a name taken, the first time — takes off everything on it, and stands the file's objects
there (`project::inserted::insert`): an id the window has taken gets the next free one, and
each support group joins the window's group of its name, a new one where there is none, the
file's profile kept as the object's own where the window's differs. That is one undo entry.
A newer revision of the job waiting takes the older one's place. A job of the same model file
takes the place of another job of it, on its plate, so a model stands once however many jobs
an agent starts on it. A plate renamed or removed is no longer the job's.

The check every tool answers with is still the command line's: the window shows its plate, it
does not answer for it.

## Agents

Each server keeps a file of its own in `agents/` beside `mcp-jobs/`, named for its process
and start, written whole — into a `.part` file, then renamed — on each tool call and every
20 s (ADR 0462):

```json
{"schema": 1, "pid": 4242, "client": "code-agent", "version": "0.9.0", "started_s": 1763460000,
 "seen_s": 1763460125, "working": {"tool": "set_hollow", "job": "cube-18e4…",
 "since_s": 1763460120}, "jobs": ["cube-18e4…", "bust-18e3…"]}
```

`client` is the name the client gave itself on connecting; `version` is the server's, and a
live agent of another version than the window's puts a row in the list offering the extension
of the window's version; `working` is there only while a
call runs; `jobs` are the jobs its calls named or answered with, latest first, at most 20.
A server that ends writes `ended_s`; one whose `seen_s` is a minute old ended without
saying. A file whose `seen_s` is a week old is taken away by the next server that starts.

The window reads the folder every 2 s off its drawing thread and lists the agents in the
agents' ring beside the print chip (`ui-screens.md`, Top bar): each live agent with the job in
its hand — the one `working` names, else the first of `jobs` (ADR 0462) — by its latest
revision — the highest `N.toml` — whose model is the first of `N.args.json` and whose
verdict is `N.json`'s `status`. An ended agent, one with no job and a job no agent names
are not listed, nor a job the user hid; one deleted from the list loses its folder
(ADR 0462). A press on a job builds the same line the server sends from
`N.args.json` (`--printer` or `--profile`, `--resin` or `--material`) and `N.encrust`,
marked `open`, and takes it as one handed over; one made on the plate is said to be there.

## Following

A line marked `"open": true` is shown whatever the window does. One not marked is shown only
while *Follow agents* is on; otherwise the window answers
`{"shown": false, "why_not": "the window is not following its agents; …"}` and the revision
waits in the list.
