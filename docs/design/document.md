# The plate document

One plate, edited one way by the window, the command line and `encrust-mcp` (ADR 0466).

## Crates

```
core-edit      Edit, ObjectId: what is asked, as data
core-document  Document, apply: what is done
```

`core-edit` names its settings by the types `.encrust` stores; it holds no algorithm.
`core-document` holds `Document` and every case of `apply`, moved down from the front ends.

## Document

`Document` is the plate as `.encrust` holds it: the manifest and each object's meshes, in the
same order, each object with an `ObjectId`. `from_project` gives an id to each object that has
none, after the highest present; `to_project` writes them. An id is never reused within a
document, so an edit naming a deleted object fails rather than landing on another.

## Edits

| Group | Variants |
|---|---|
| placement | `Move`, `Rotate`, `Scale`, `Orient` |
| plate contents | `Delete`, `Duplicate`, `Arrange` |
| repair | `Repair`, `Mend`, `Relief` |
| shaping | `Combine`, `Label`, `Cut`, `Split`, `PartOff` |
| hollow and drains | `Hollow`, `Drain` |
| supports | `Supports`, `ShownAt` |

Points and positions are plate millimetres. An edit names the objects it acts on; one that
adds objects answers their ids and what each is (`Applied`), so a front end can name or select
them. Print settings, printer and resin are a revision's, not edits.

## apply

`apply(&mut Document, &Edit, Cancel)` changes the document or leaves it as it was: an error
never leaves half an edit. A caller that cannot cancel passes `Cancel::never()`.

Until the window's `Scene` is a `Document`, a front end runs an edit's steps rather than
`apply`: the window's Cut and `--cut` call `cut` and `pieces`, its Split and `--split` call
`split` and `part_off_at`, the same calls `apply` runs.

`Cut { object, axis, at_mm, keep, joints, lay_flat }` cuts the object's printed mesh, its
drain holes taken out first, at `at_mm` along `axis` on the plate. The kept halves replace it
with no supports or cavity of their own; with `lay_flat` each lies on its cut face, except a
lower half carrying pins or a key; loose dowels stand in a row beside the lower half. A plane
that misses the object is `ApplyError::Missed`. The window runs the two steps on a worker,
`--cut` on a mesh it has not made an object yet; `--cut` always lays flat.

`Move { object, by_mm }` adds to the translation. `Rotate { object, degrees }` sets the turn
(X, then Y, then Z of the plate, as `--rotate`) and `Scale { object, factors }` sets the
scale, each about the object's centre of mass and then standing it on the plate; a factor not
finite or above zero is `ApplyError::BadScale`. None of them adds or removes an object
(`Report::Placed`). The window sets the transform at once through the same `placement`
functions.

`Split { object, smallest_mm3 }` and `PartOff { object, at_mm }` run on the object's printed
mesh placed on the plate and replace it with `Role::Part(n)` pieces, counted from one: a split
by `core_volume::parts` (the default smallest part where unset), a part-off by
`core_volume::part_off` into the part and the rest. A split that finds one body and no scrap
joined or dropped is a no-op: the object keeps its id and `Report::Split` says so. Errors map
as `NoNeck | NotOnModel` to `ApplyError::NoNeck`, a stopped run to `Cancelled`, any other to
`Failed`. `apply` takes a `core_volume::Cancel` (ADR 0466's `Run`).

`Delete { object }` removes the object and the plugs laid for it. `Duplicate { object,
offset_mm }` puts a copy with all its state and meshes right after it, under a new id, a plug
of nothing, its translation moved by the offset (`Role::Copy`). `Mirror { object, axis }`
negates the scale on that axis of the object's own frame and moves it back so its bounds'
centre stays. `plug_of` is an index among the objects, so `Document` renumbers it on every
removal and insertion: a plug of a removed object names nothing.

`Combine { objects, op, toward }` replaces the objects by one `Role::Combined` piece at the
first's place, made by `core_document::combined` on their source meshes placed on the plate: the
exact `combine_exact_toward` where `toward` is set, else `core_volume::combine` (sampled where an
object is not a closed solid, `Report::Combined { voxel_mm }`). Fewer than two distinct objects
is `ApplyError::TooFew`, a missing one `NoObject`, an empty result `NothingLeft`, an unreadable
first `NotSolid`.

`Label { object, text, at_mm, height_mm, depth_mm, raised, follow_surface, angle_deg, font }`
writes `text` on the surface of the object nearest plate point `at_mm`, the object's source
placed on the plate and stamped by `combined` toward the face's normal; the result replaces it
where it stands (`Role::Kept`, `Report::Labelled`). `height_mm` and `depth_mm` unset are core-text's
`LETTER_HEIGHT_MM` and `LABEL_DEPTH_MM`; `raised` defaults to true, `follow_surface` and `angle_deg`
to flat and 0. `font` is a file, the bundled IBM Plex Sans where unset. `label_prisms` lays the
letters (the window runs it, then the stamp on a job); `labelled` is both. Errors: `BlankLabel`
(no outline in the text), `NoSurface` (no face near `at_mm`), `BadFont { path, reason }`, the
stamp's own `NothingLeft`, `NotSolid` and `Failed`.

`Repair { object, method }` runs `core_document::repaired` (`core_volume::repair`, the window's
default `RepairSettings`) on the object's source mesh and `Mend { object, flip_faces, drop_faces,
drop_shells, bridge }` runs `mended`, which `--flip-faces` and its kin call too: turns and bridges
first, drops last. Both end in `Document::reshape`, which replaces the source and takes off what
was built on the old surface (supports, cavity, painted regions; plugs stay, as the window keeps
them); the object keeps its id (`Report::Repaired`, `Report::Mended`). Errors: a face past the
last `NoSuchFace` (from 0), a shell `NoSuchShell` (from 1), a bridge the mesh refuses `Failed`,
`Cancelled`. The window runs a repair as its Repair tool's job and a mend as one job of the Faces mode.

`Relief { object, depth_mm, image, tile_mm, angle_deg, invert, symmetric, smoothing_px,
hold_bottom_deg, hold_top_deg, precision }` presses a picture tiled over the object, the
options defaulting as `--relief`'s flags do; `relieved` is what the CLI's `--relief-image` runs
too. core-document reads the picture through core-mesh-io's `Texture::decode`, its one reader.
The press is at the size the object prints at, so its scale and stretch go into the mesh, which
`Document::reshape` replaces (`Report::Relieved`); the faces painted for relief limit it where
any are. A picture that does not read is `BadPicture { path, reason }`, else `Cancelled` or
`Failed`. The model is left solid, its cavity gone, until Hollow is an edit. A file's own texture is not an edit: a `.encrust` keeps none. The window's Relief tool
runs the edit as its own job, `press_picture` being the call both make.

## Undo

The window keeps snapshots of its scene (ADR 0086); a document applied is one entry. The
window's `version` counts recorded entries, undo and redo, for the link
(`window-link.md`).
