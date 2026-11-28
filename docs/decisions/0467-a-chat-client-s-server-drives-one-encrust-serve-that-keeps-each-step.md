# 0467. A chat client's server drives one `encrust serve` that keeps each step by its hash

- **Status:** Accepted
- **Date:** 2026-11-28

## Context

ADR 0455 ran a process of the command line per call, so each check staged the model again:
on a 1.4-million-face statue cut in two, posed, hollowed, drained and supported, one session
took about seven minutes of runs, a full stage 22 to 27 s whatever one step changed. ADR 0464
answered it with a second path, edits on the plate a revision kept, used where every change
was an edit; any other change, taking a drain point back among them, staged the file anew.
The two paths searched the pose on different plates and stood the halves differently, so a
point read off one revision's check landed elsewhere on the next, and a drain point the client
copied from the report was refused as off a face. `compare_poses` listed the first object's
ways and `set_pose` took the same number for every piece. Times showed where a run goes: on
that statue, the stages of a warm run (the pose kept, ADR 0466) were hollow 5 s, drains 4 s,
supports 3 s, laying the parts 3 s, the check 3 s, import 0.8 s, the project written 0.7 s.

## Decision

- **`encrust serve`** reads a run a line, `{"id", "args"}`, runs it on a thread of its own with
  `--json` as the command line would, and answers `{"id", "exit_code", "report"}`;
  `{"cancel": id}` stops a run between its steps. A panic answers that run as failed. The
  server keeps `Steps` between runs; a run of the command line keeps none.
- **Every edit is kept by a chained key.** A plate's key is the SHA-256 of what staged it — each
  file's bytes, what its import and held supports ask, the machine — and each edit's key is the
  key before it and the edit, a font or picture it reads by its bytes. `Staging::apply_with`
  answers an edit asked again from the plate kept, so a run that changes its tenth step pays
  for the tenth step onward. Files are read once while their length, time and ends stand; a
  project the server wrote is taken back as the plate it wrote, wherever it was moved. A model
  is cut once per object state, keyed by its state, its meshes and the support groups; a
  check once per plate, by its models and every setting it reads. Each kind keeps its most
  recently used, a few dozen at most.
- **One path without a window.** The server stages each revision by its flags; the edits on a
  kept plate (ADR 0464) are gone, and edits remain for the open window's plate (ADR 0456), a
  plate of pieces never edited as one model. A revision of several runs lays the earlier ones
  with `estimate --stage-only`, which writes the plate and measures nothing.
- **Points stay on their object.** A new job's recipe keeps drain, channel, kept-solid, raft,
  support, shown and merge points in each object's own frame, carried there from the plate by
  `models[].frame.from_plate` of the latest check; the command line, by `--points-in-object`,
  carries them back where the object stands when the edit runs. A job begun before keeps
  plate points.
- **Poses by object.** `compare_poses` lists each object's ways under `objects`; `set_pose`
  takes `picks` per object over `pick`, as `--orient-pick=N,ID:K`.
- **What is not the client's wait is not in it.** Each revision is handed to the open window
  aside, in a queue run in order, and the answer says `"shown": "pending"`: the window repeats
  each edit on its own plate, as long as the check took or, built for debugging, far longer. What
  it did not take comes with the job's next answer as `window_before`, the revision standing on
  its own plate, shown in its place. A revision while the window is still taking one is staged
  by its flags, not built on the window's plate. `start_job` keeps the autopilot's check when it
  ends, aside; jobs untouched past 30 days go, then the oldest past 4 GiB, none
  touched that day. A client that takes the MCP tasks extension gets a long call as a task to
  poll and cancel; one that does not, the call with its progress, as before.
- **`stages_s`** in the estimate report gives each stage's seconds.

## Consequences

The session above runs in about 130 s of runs, not 430, and its answers no longer wait the
15 to 37 s a step the window took to repeat it; a step after the first costs its own
work and a check, a check of a plate asked again nothing. Determinism is now load-bearing:
an edit whose result depends on anything but the plate and itself — the time, a file it reads
but the key leaves out — would be answered stale; a new input of `apply` must reach the key.
The server holds plates in memory, bounded by count, not bytes. A support profile changed
stages everything after import again, since the plate holds it from the start. Reopen when
memory runs short on large plates, or when the window's own plate should be the one path.

## Alternatives considered

### The command line in process

No second process — but a panic would end the server and stdout is the protocol (ADR 0455).

### Keep the piece-by-piece path beside the cache

Appends stay cheap either way — but two paths stand the model two ways.

### A check cached per object, layer by layer

Each object's cured spans for 1600 layers are about 260 MB on that statue: more memory a state
than the half of a 3 s check it saves.

### Answer once the window holds the revision

The answer told what the window did with it, and a revision the window refused was not kept —
but every step waited for the window to repeat it, on that statue 15 to 37 s a step.

### The option that won, and what it costs

A protocol, a cache and its keys to keep in step with `apply`, and a server process that
holds gigabytes on a large plate until it is stopped.
