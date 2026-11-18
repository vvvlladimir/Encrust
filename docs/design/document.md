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

`apply(&mut Document, &Edit, Run)` changes the document or leaves it as it was: an error
never leaves half an edit. `Run` carries the cancel flag and the progress a window job shows.

`Cut { object, axis, at_mm, keep, joints, lay_flat }` cuts the object's printed mesh, its
drain holes taken out first, at `at_mm` along `axis` on the plate. The kept halves replace it
with no supports or cavity of their own; with `lay_flat` each lies on its cut face, except a
lower half carrying pins or a key; loose dowels stand in a row beside the lower half. A plane
that misses the object is `ApplyError::Missed`.

## Undo

The window keeps snapshots of its scene (ADR 0086); a document applied is one entry. The
window's `version` counts recorded entries, undo and redo, for the link
(`window-link.md`).
