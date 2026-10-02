# 0086. Take an edit back by restoring a scene, not by inverting it

- **Status:** Accepted
- **Date:** 2026-09-24

## Context

The plate is about to grow the tools of step 11 — copies, mirrors, arrays, orientation,
arranging, cutting — on top of the seven that already write to the scene: the gizmo, the
transform fields, support points, blockers, drain holes, channel points and the jobs that
land hollowed shells and generated supports on a later frame. Every one of them needs to be
undoable, and the ones that arrive from a worker thread have no click to hang an inverse
operation on.

A `SceneObject` holds its mesh, its hierarchy, its cavity and its support columns behind
`Arc`. Cloning the whole `Scene` therefore copies placements, point lists and reference
counts: kilobytes for a plate of six models, whatever the triangle count.

## Decision

`History` keeps whole `Scene` clones — up to `DEPTH` of them — and an edit is taken back by
putting one of them back. Nothing writes an inverse operation.

What counts as an edit is found rather than declared: `observe` hashes the scene's
*inputs* — which models are on the plate, where they stand, whether they are hollow, and
every point the tools have placed — and records a snapshot when that hash changes. Meshes
built from those inputs are left out of the hash, because they land a frame or two after
the edit that asked for them and would otherwise make one drag two entries.

A snapshot is taken only on a frame with no mouse button down, so a drag across the plate
is one entry rather than sixty.

## Consequences

A tool added later is undoable the day it is written, including one whose work finishes on
a worker thread. The window keeps one extra `Scene` beside the stack, and `DEPTH` of them
behind it; on a plate of six models that is tens of kilobytes.

The cost is that undo is only as complete as the hash. A field added to `ObjectHollow` or
`ObjectSupports` that nobody adds to `fingerprint` is silently not undoable — the edit
happens, and Ctrl+Z steps over it to the one before. That is one function to keep honest
rather than an inverse per tool, but nothing fails when it is forgotten.

The signal to reopen: a plate big enough that a clone per edit is felt, or a tool whose
state genuinely does not belong to the scene.

## Alternatives considered

### A command stack, one inverse per operation

Memory proportional to what changed, and the usual answer. Rejected because every tool
would have to carry its own inverse, including the jobs that mutate the scene from a
thread, and a missing inverse corrupts the history rather than skipping an entry.

### Snapshot on an explicit call before each mutation

Exact: no hash to keep in step. Rejected because the call sites
are the same seven-and-growing places a command stack would need, the background jobs among
them, and a forgotten call fails exactly the way a forgotten hash field does.

### The decision above, and what it costs

Undo is driven by a hash of the scene rather than by the code that edits it, so the history
is correct by observation and not by construction. A field left out of `fingerprint` is
invisible until someone tries to undo it, and no test that does not know about the field
will catch it.
