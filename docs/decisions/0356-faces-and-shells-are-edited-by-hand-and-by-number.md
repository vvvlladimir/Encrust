# 0356. Faces and shells are edited by hand in the window and by number on the command line

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

A repair method (ADR 0351, 0352) fixes what it can tell is wrong. What it cannot tell —
a loose piece that is meant to go, a patch of faces turned the wrong way on purpose by
an exporter, a gap two edges should close — needs the user's eye. The Repair tool already
finds holes, lists what is wrong and flies to it (ADR 0354, 0355); the window could not
yet act on one place. The command line has to reach whatever the window does (rule 9 of
`architecture.md`), but has no pointer to pick with.

## Decision

- **Four core calls in `core-geometry::repair::edit` and `shells`:** `flip_faces` and
  `remove_faces` over a `Region`, `bridge` between two open edges given by their vertices,
  `list_shells` and `remove_shells` by their place in that list. `drop_small_shells` is now
  `list_shells` filtered, so the area and openness of a shell are worked out once.
- **`bridge` winds its faces from the faces beside the edges:** each open edge is walked the
  way its one face does not, so a quad cannot twist; apart, the diagonal of less area is
  taken; edges meeting at a vertex take one face. It refuses with `BridgeError::NotOpen`,
  `SameEdge` or `Degenerate`, which is why `core-geometry` now depends on `thiserror`, as
  every other core with an error does.
- **The window:** the Repair card gains a Faces mode beside Model. Faces are picked with a
  brush, or a whole surface with Cmd and Shift, as Paint picks (ADR 0318), then Flip or
  Delete; or two open edges are picked by a click within 10 points on the screen, the
  nearest one no deeper than the surface under the cursor first, else the nearest at all,
  then Bridge: depth orders what is in reach and never empties it, as a ray to a rim that
  thin grazes the faces round it. The edge a click would take is lit under the pointer,
  and a click near none says so. Model gains a Shells list for one model:
  a row picks the shell and flies to it, its bin drops it; two icons in the heading pick
  every shell under a hundredth of the largest one's area, or turn the pick over. The
  smallest speck of the repair is no measure here: a loose shell is often far larger. Every change runs off the drawing thread and lands as one undo.
- **The command line, by number:** `--flip-faces`, `--drop-faces`, `--drop-shells`,
  `--bridge A-B:C-D`, every number of the model as import leaves it, which is also the mesh
  the window holds; turns and bridges go first and drops last so no number moves under one
  to come; `inspect --shells` lists the shells as `--drop-shells` numbers them.

## Consequences

- Whatever a repair method gets wrong can be put right without leaving the window, and
  the same change can be scripted.
- Face and vertex numbers are a contract between `inspect` and the flags: a change to what
  import does to a mesh before them (welding, dropping doubled faces) renumbers them.
- A picked shell of more than 100 000 faces is flown to but not marked, its patch being
  laid on the drawing thread; a surface picked whole is laid there too. Reopen if either
  is felt as a hitch.
- Picking an open edge looks at every open edge of the targets on each click, which is
  far fewer than the faces; past 200 000 of them the edge under the pointer is not lit
  while hovering, and a click on an unwelded model is slow.

## Alternatives considered

### Pick faces on the command line by a box or a point

Closer to how the window picks, but a second picking language to learn and document; the
numbers `inspect` prints are already the model's own.

### A bridge built as the user clicks, wound as the clicks run

What a viewer that does not read the surface does, and why its quads twist when the
edges are clicked crosswise. The faces beside each edge already say which way it goes.

### The option that won, and what it costs

A new dependency for `core-geometry`, small and already in the workspace; and numbers on
the command line that only mean something against `inspect`'s own listing.
