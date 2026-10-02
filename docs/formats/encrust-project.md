# The Encrust `.encrust` project

Our own format: a plate as it was left, so that it opens the same tomorrow. Written and
read by `encrust-app` alone, in `crates/encrust-app/src/project.rs`.

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
the build reads is refused whole rather than parsed as far as it goes. A key this build
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

Anything the window can work out again: bounding hierarchies, support trees, the hollow
shell and its cavity, cut bodies, the slice stack, the layer preview, the undo history.
Opening a project leaves the plate in the state a model is in the moment it is imported —
the supports regrow on the next refresh, and the cavity is asked for again by the Hollow
tool. See `docs/decisions/0097`.
