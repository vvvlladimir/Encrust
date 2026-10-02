# 0038. Import models on a worker thread

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

Opening a model read the file, welded it, fixed its orientation, diagnosed it and built
its hierarchy inside `SlicerApp::open_model`, which runs inside `eframe`'s update. Every
one of those is proportional to the triangle count, and a frame is 16 ms.

Measured with `cargo bench -p core-geometry` on a 200k-triangle sphere, after the repair
work was sped up: weld 30 ms, orient 12 ms, diagnose 18 ms, hierarchy 32 ms. That is 92 ms
of work on a mesh smaller than a detailed miniature, before the file is even read. A
880 604-triangle part is several times that, and welding alone took 1.53 s before it was
rewritten. The window did not paint, did not respond to the close button and gave no sign
of what it was doing — on a real part, for seconds.

Slicing, previewing and support placement already solved this: each owns a job that runs
on its own thread and reports over an `mpsc` channel, and `SlicerApp::update` polls them
all and asks for another frame while any is running (ADR 0018). Import was the last piece
of heavy work still on the drawing thread.

## Decision

An import runs on its own thread. `ImportJob::spawn` takes the path and a copy of the
plate, runs `import::prepare` there, and sends back a stage — reading, repairing,
indexing — as each part starts, then the finished `Imported` or the flattened error chain.
`Imports` holds every running job, is polled once a frame beside the other jobs, and puts
what has finished into the scene.

`prepare` returns an `Imported`: name, mesh, hierarchy, transform and repair summary. That
type is now the only way a mesh reaches a `SceneObject`, so `Scene::insert` takes one and
builds nothing itself. The hierarchy is therefore built on the worker that made the mesh.

The thread is detached and an import cannot be cancelled. Several files opened at once
each get a thread, because a dropped selection is a handful of files, not hundreds.

## Consequences

The window keeps painting, keeps orbiting and says which file is at which stage while a
model opens. The camera frames what arrived on the frame it lands, as before.

`import::open_path` and `import_into` are gone; the panels that used to call them call
`Imports::open_dialog`. A test that opens a model now polls `Imports` until it settles
rather than reading the scene on the next line, which is the honest shape of the thing.

An import cannot be stopped, so a wrong 2 GB file is read to the end — it just no longer
freezes the window while it happens. A cancel flag is worth adding if that is ever a
complaint in practice.

Nothing bounds how many imports run at once. Opening thirty files would start thirty
threads and thirty welds. If multi-file import becomes a real workflow, this wants the
same thread budget slicing has (ADR 0019).

## Alternatives considered

### Keep it on the UI thread and draw a progress bar

A progress bar drawn by a thread that is not painting shows nothing. To keep the window
alive the work has to leave the thread; there is no version of this that stays.

### One import thread, with a queue

Bounds the thread count for free and makes the second of two files wait for the first.
Two large files opened together would then take the sum of their times with the machine
half idle, and the bound it buys does not matter at the file counts a person drags onto a
window.

### Slice the work across frames instead

Weld a hundred thousand vertices per frame, keeping everything on one thread and cancellable
by construction. It would mean threading a resumable state through `weld`, `orient_outward`
and `Bvh::build` — three algorithms that are written as straight loops and are much clearer
that way — to avoid a thread the rest of this application already uses everywhere.

### The option that won, and what it costs

An import now crosses a thread boundary, so the mesh is built in one place and consumed in
another, and the scene changes on an arbitrary later frame rather than inside the click
that asked for it. Tests have to poll. The detached thread means a model that fails late
is reported late, and a model whose window closes mid-read finishes its work into a
dropped channel and throws it away.
