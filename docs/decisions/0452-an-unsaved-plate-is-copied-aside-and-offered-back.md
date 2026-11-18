# 0452. Copy an unsaved plate aside every two minutes, and offer it back after a run that did not close

- **Status:** Proposed
- **Date:** 2026-11-16

## Context

A plate is only on disk once the user saves it. A run that ends without closing — a panic,
a GPU driver abort, a kill, a power cut — loses everything since the last save, which after an
hour of placing supports by hand is an hour. ADR 0446 recovers the panic's message, not the
work. A crash cannot be relied on to run any code, so the plate has to be on disk before it.

Several windows run at once on Windows and Linux, each its own process, so "the last run" is
not one run, and a run still open must not have its copy offered to another.

## Decision

`encrust_app::autosave` keeps one slot per run in `recovery/` beside the user's profiles:
`<pid>-<start ms>.lock`, held with `File::try_lock` for as long as the run lives, beside the
copy `.encrust` and a `.belonged` file naming the project it came from. Every two minutes the
window captures the plate as Save does, and a plate that differs from its file and from the
last copy is written by a thread of its own: whole to a `.part` file,
`sync_all`, then renamed over the copy, so a run cut off mid-write leaves the copy before it.
A plate that matches its file, or holds no model, drops the copy. A clean exit deletes the
slot.

At start a lock the window can take is a run that is gone: its copy is offered, newest first,
by a notice while the plate is empty. Restore opens it as the unsaved plate it was, under the
file it belonged to, and renames it into the run's own slot; the cross discards it at a
second press. Settings → General turns copying off. The browser keeps no copy.

## Consequences

A run that ends badly loses at most two minutes. No crash handler, signal or flag is needed:
the lock goes with the process however it ends. A plate is written in full every time it
changes, which for a scan of a million faces is a few tens of megabytes every two minutes, off
the drawing thread; the capture itself stays on it, as Save's does. A copy not restored or
discarded is offered again at every start.

Reopen if a capture is found to take longer than a frame, or a write every two minutes to wear
a disk users care about.

## Alternatives considered

### Write over the project file itself

What other programs call autosave. It saves a change the user may not want and does nothing
for a plate never saved, which is the plate most often lost.

### A flag written at start and deleted on a clean exit

What tells a crash from a close, but not which of several runs crashed, nor whether a run
whose flag is still there is gone or only still open.

### The copy at a fixed step, and what it costs

Copying on every edit loses nothing, but writes a whole plate per brush stroke. Two minutes is
a choice of loss against writes, not a measured one; the user can only turn it off, not tune it.
