# 0018. Slice on a worker thread, report over a channel

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

Step 5c slices from the window. A real model is thousands of layers, and the slice plus
the rasterisation of one takes minutes: running that inside `SlicerApp::ui` would freeze
the window for the whole job, with no progress and no way to stop it.

The work itself already exists. `PlaneSliceEngine`, `ScanlineRasterizer` and `GooWriter`
are plain library code with no window in them, and `encrust-cli` already sequences them:
slice, then rasterise a window of layers in parallel and push each one into the file.
What the window adds is that the sequence has to run somewhere other than the UI thread,
has to say how far it has got, and has to stop when asked.

The scene holds several objects, each with its own placement, and the slicer takes one
mesh.

## Decision

`encrust-app` runs one job at a time on a `std::thread`, and the window talks to it through
two primitives only:

- an `mpsc::channel` carrying `Progress` messages from the worker: the stage it has
  reached, how many layers of how many have been written, and the `Outcome` it ended on;
- an `Arc<AtomicBool>` the window sets to cancel. The worker reads it after slicing and
  between raster windows, so cancellation lands within one window of layers.

`SliceJob` is the handle. It is drained once a frame by `Slicing::poll`, which keeps the
last counts and hands the terminal outcome to the status bar. The window requests a
repaint on every frame a job is alive, because a channel wakes nothing on its own.
Dropping the handle cancels the job: a worker with nobody to report to has no reason to
keep writing a file nobody asked for any more.

The worker is `job::pipeline::run`, a plain function of a `SliceRequest`, an
`&AtomicBool` and a `&mut dyn FnMut(Progress)`. It takes no window state, so the whole
pipeline is tested synchronously, cancellation included, with no thread and no frame.

The job writes a `.goo` file. The PNG stack stays a CLI debug artefact, as in ADR 0011.

Every visible object is baked into one mesh in plate coordinates before the request is
built (`job::merge`), because two overlapping models are one solid on the plate.

A cancelled job removes the file it had started: a `.goo` that stops half way through the
stack is a valid-looking file that prints a truncated part.

The `encrust-cli` and `encrust-app` pipelines stay separate. What they share is a sequence
of calls into the core traits, about thirty lines, not a type; extracting a `core-pipeline`
crate for it would buy a layer of indirection and no reuse. The one thing that was
genuinely shared — the resin volume of a sliced stack — moved down into
`Sliced::resin_volume_mm3`, where both callers read it.

## Consequences

- The window stays responsive: the frame loop only drains a channel and draws a bar.
- Progress is reported per raster window, not per layer, so the bar advances in steps of
  as many layers as there are threads. That is the same window the memory bound in ADR
  0010 is built on, and a finer report would mean a channel message per layer.
- Cancellation granularity is one raster window. On a huge panel with few threads that is
  a visible pause between the click and the job stopping, which is why the button shows
  "Cancelling" rather than disappearing.
- One job at a time. The Slice button is replaced by the progress bar while a job runs,
  so a second one cannot be started. Queueing several outputs would need a different
  handle, and nothing asks for it yet.
- The mesh is copied into the request, so the user can keep moving models while a job
  runs, and what lands in the file is what was on the plate when Slice was pressed.
- The thread is detached. There is no join on shutdown: if the window closes mid-job the
  process exits with a partial file on disk. Closing the window is not a cancel and the
  file is not cleaned up in that one case.
- `encrust-app` now depends on `core-raster`, `format-goo` and `rayon`, all of which the
  allowed dependency graph already permits for a binary crate.

## Alternatives considered

### Slice on the UI thread, in chunks, driven by the frame loop

A few layers per frame, state kept in the app struct, no thread at all. Rejected: it makes
the frame rate a function of how long a layer takes, it cannot use rayon inside a frame
without stalling the paint anyway, and it turns a linear pipeline into a state machine
that has to be re-entered in the middle of a `for` loop.

### A shared `Arc<Mutex<JobState>>` instead of a channel

Simpler to write for a single value, but it invites reading half-updated state, needs a
lock in the paint path, and offers no way to say "this message is the last one". The
channel already carries ordering and end-of-stream, which is exactly the shape of a job
that reports and then finishes.

### Extract the pipeline into a shared crate for both binaries

The CLI and the window now both call slice, rasterise and write in the same order.
Rejected for now: the CLI's version is interleaved with reporting types that only make
sense on a terminal, and the common part is a call sequence rather than a type. Rule 5 of
the architecture rules forbids duplicating a *type* across crates, which this is not. The
signal to revisit is a third caller, or the first divergence in behaviour between the two
that turns out to be a bug rather than a choice.

### The option that won, and what it costs

Threads and channels are the plain answer, but they leave real edges. The window cannot
tell a hung job from a slow one, since there is no timeout and no heartbeat between raster
windows. Progress is a count of layers, not of work: the last layers of a tall model are
usually the cheapest, so the bar advances unevenly. There is no pause and resume. A worker that
panics is seen by the window only as a channel that closed without an outcome: the job is
reported as failed, but the message says nothing about what the worker was doing, because
the panic itself goes to the log and not through the channel. If that ever matters in
practice the worker will have to catch the unwind and report the payload.
