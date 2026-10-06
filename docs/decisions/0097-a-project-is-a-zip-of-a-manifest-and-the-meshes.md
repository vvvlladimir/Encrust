# 0097. A project is a zip of a JSON manifest and the meshes, holding only what was asked for

- **Status:** Superseded by 0191
- **Date:** 2026-09-26

## Context

Step 14 wants the plate saved and opened again. What is on a plate now is a scene of
models, a printer and a resin, exposure bands, adaptive rules, support groups with their
profiles, per-model support points, two painted patches, frozen support trees, hollow
blockers, drain holes and channels. Some of that is what the user asked for; most of the
bytes in memory are what the window built from it — hierarchies, support meshes, the
cavity, the slice stack.

The patches are the constraint that decides the mesh format. A `Region` is a set of face
indices (ADR 0092), so any round trip that renumbers or drops a face silently moves every
patch on the model. Re-welding on load does exactly that.

## Decision

A `.encrust` project is a zip: `project.json`, and `models/<n>.mesh` per object in
manifest order. Layout in `docs/formats/encrust-project.md`.

The manifest is JSON, from `serde` derives on the types the code already has, so a field
is persisted by existing where it already exists rather than by being mirrored. Those
derives are always on, not behind a feature: a feature that CI does not enable is a
serialisation nobody tests.

A mesh is our own blob — magic, counts, then the vertex and index buffers — written and
read verbatim.

Only what was asked for goes in the file. Everything derived is left out and rebuilt: an
opened plate has no support trees, no cavity and no stack, exactly as a freshly imported
model does.

## Consequences

A project is small and compresses well: two models of 400k triangles come to what their
STLs would, and every setting is a few kilobytes beside them. A field added to a tool is
persisted by adding it to the manifest struct, in one place.

Opening costs a bounding hierarchy per model. They are built across `rayon`, but on the
calling thread, so a plate of six large models holds the window for a beat. If that
becomes the complaint, it is the same job plumbing an import already has.

Re-hollowing after opening is a click the user did not have to make before saving. We
accept it: storing the shell would roughly double the file for something a second of work
reproduces exactly, and the Hollow panel already shows a cavity as stale.

Reopen this if a project ever has to be read by something other than the window — a
batch run, or another program. Then the mesh blob is the part to revisit, because nothing
else reads it.

## Alternatives considered

### 3MF, the format every FFF slicer writes

A real standard with a settings extension, and models would open in other tools. But
nothing outside this workspace can read our supports, patches, channels or blockers, so
the XML would carry our data in a namespace of our own anyway — the standard's shape for
none of the standard's benefit, plus an XML dependency.

### STL for the meshes inside the zip

Already implemented and already a dependency. Rejected outright: it is triangle soup, so
loading one means welding it, and welding renumbers faces. Every painted patch and every
blocker would land somewhere else on the model.

### One file, with the meshes base64 in the JSON

No zip dependency. A third more bytes, a manifest no editor will open, and no way to read
one model's geometry without parsing the whole document.

### A `serde` feature on each core crate

Keeps serde out of a build that does not want it. It also keeps the round-trip tests out
of `cargo test --workspace`, which is the command this project gates a step on, so the
impls would go untested until something broke in the window.

### The option that won, and what it costs

Two new dependencies, `zip` and `serde_json`, and serde derives now sitting on data types
in five core crates that have nothing to do with files. The manifest is also a format we
have to keep reading: every field added from here is a field version 1 will not have, so
`Option` and `#[serde(default)]` become the habit, and the discipline of refusing a
newer file is only as good as the version number being bumped when it should be.
