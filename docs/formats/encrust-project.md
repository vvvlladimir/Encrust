# The Encrust `.encrust` project

Our own format: a plate as it was left, so that it opens the same tomorrow. Written and
read in `crates/core-engine/src/project.rs`, which has no front end in it, so a window,
a terminal and a browser all open the same file (ADR 0174). What the window converts to
and from it is `crates/encrust-app/src/project/state.rs`.

## Shape of a file

A zip. Deflate, no encryption, no comment.

```
project.json        the manifest, UTF-8
models/0.mesh       the first object's geometry
models/1.mesh       the second's
...
```

`models/<n>.mesh` matches `objects[n]` of the manifest, by position. A manifest with more
objects than the file has meshes is refused.

## `project.json`

`version` is read on its own before anything else, and a file claiming a higher one than
the build reads is refused whole rather than parsed as far as it goes. This build writes
version 2, which added `cavity`; a version 1 file opens with every model solid. A key this build
added since is `#[serde(default)]`, so a file written before it still opens: a project
with no `plates` is a project of one, with everything on it. Every other field
is the serde form of a type the code already had:

| Key | Type |
|---|---|
| `printer`, `resin` | `{ id, profile }`, the catalogue id and the profile itself |
| `slicing` | layer height, `AdaptiveSettings`, `ExposureRange` bands, anti-aliasing, `OutputFormat` |
| `supports` | the group table, the active group, `ProjectSettings` for a fill, the brush |
| `hollow` | what the Hollow tool is set to, not what it built |
| `drain`, `cut`, `array` | what the next hole, the cut plane and the copy grid are set to |
| `plates`, `active_plate` | one name per build plate, and the one the project was left on |
| `objects[]` | name, plate, `Transform`, visibility, the import summary, and what each tool placed |
| `objects[].hollow.cavity` | the wall a model was hollowed to — thickness, mode, precision, infill — or `null` for a solid one |

The profile travels with the file, not only its id: a plate has to open the same on a
machine whose catalogue never had that printer. The id is kept beside it so the picker
still shows the profile as the catalogue's when it is.

## `models/<n>.mesh`

```
"ENCM"          4 bytes
vertex count    u32 le
face count      u32 le
vertices        f32 le x3, per vertex
faces           u32 le x3, per face
```

Not STL, and not re-welded on load. A painted patch is a set of **face indices**
(`core_supports::Region`, ADR 0092), so a format that renumbered or dropped a face would
move every patch on the model. The index buffer is written as it stands.

## What is not in the file

Anything that can be worked out again: bounding hierarchies, support trees, the hollow
shell, cut bodies, the slice stack, the layer preview, the undo history. The supports
regrow from their points and each shell is built again from its `cavity`, by the window on
opening and by `core_engine::open_plate` for anything without one. See
`docs/decisions/0097` and `0178`.
