# 0015. The window owns the scene; meshes are shared behind `Arc`

- **Status:** Accepted
- **Date:** 2026-09-17

## Context

Step 5a gives `encrust-app` something to hold: a list of imported models, each with its
repair summary and its placement on the plate. Three consumers want that mesh data.

The UI reads it every frame to list objects and their bounds. The renderer uploads it once
and then wants nothing but the placement matrix. Step 5c will hand it to a background
thread that slices while the window keeps drawing. A mesh is millions of vertices; a
Mars 4 Ultra print is not unusual at ten million triangles, so a per-frame or per-thread
copy is not affordable.

Placement is different. It is twelve floats, it changes on every frame of a gizmo drag in
step 5b, and both slicing and rendering need the current value rather than a snapshot.

## Decision

`SlicerApp` owns one `Scene`, which owns a `Vec<SceneObject>`. A `SceneObject` holds its
mesh as `Arc<Mesh>` and its placement as a plain `Transform`, and the mesh inside that
`Arc` is never mutated: an edit that changes geometry produces a new `Arc`.

Objects are identified by an `ObjectId`, a counter that is never reused, rather than by
their index in the vector. Removing an object does not renumber the ones after it, and a
selection or a queued job that names a deleted object resolves to nothing instead of to
the wrong model.

The renderer caches GPU buffers in a map keyed by the mesh's `Arc` address, and holds a
clone of the `Arc` in each cache entry. Holding the `Arc` is what makes the key sound: the
allocation cannot be freed and the address cannot be handed to a different mesh while an
entry still refers to it. Entries not drawn in a frame are dropped at the end of it.

## Consequences

- The renderer uploads a mesh once and re-uploads nothing while it is only being moved.
  Placement reaches the GPU as an instance row, rebuilt every frame, which is cheap.
- A background slicing thread in step 5c takes `Arc::clone` and needs no lock, because the
  mesh behind it is immutable. Only the `Transform` has to be copied under the UI's
  ownership at the moment the job starts.
- Two objects that share a mesh — a duplicated model — share one GPU buffer for free.
- Mesh editing is not supported. Anything that changes geometry, such as merging supports
  into a model before slicing, builds a new `Mesh` and a new `Arc`. That is the intended
  shape, but it means there is no in-place vertex edit path and adding one later would
  invalidate both the cache key and the thread-safety argument above.
- `ObjectId` lookups are a linear scan. With the number of objects a plate holds that is
  not worth a map; if a plate ever holds hundreds, the scan is the thing to change.

## Alternatives considered

### Index into a `Vec`, no identifiers

Simplest possible. Rejected because removal renumbers everything after the removed object,
and every stored index — the selection, a gizmo drag in progress, a running slice job —
silently starts pointing at a different model. That class of bug is invisible until a user
deletes the wrong thing.

### One flattened mesh for the whole plate

Slicing eventually wants every object merged anyway, and a single vertex buffer draws in
one call. Rejected because it destroys per-object identity: selection, per-object
transforms and per-object diagnostics all need the objects kept apart, and re-flattening
on every gizmo drag would rebuild the whole buffer.

### `Arc<Mesh>` with an address key, and what it costs

An address is a fragile key. It is only correct because the cache holds the `Arc`, and
that invariant lives in a comment and in this document rather than in the type system — a
future refactor that stores a `Weak` or drops the `Arc` to save memory would reintroduce
exactly the reuse bug the `Arc` prevents. Keying on `ObjectId` instead would be robust
without that argument, but it would give two copies of the same mesh two GPU buffers and
would re-upload a mesh that merely changed owner. We took the sharper key and wrote down
why it holds.
