# 0101. A tool works on what is picked, and on the whole plate only when nothing is

- **Status:** Accepted
- **Date:** 2026-09-26

## Context

The scene held one selection, `Option<ObjectId>`, and every tool that does work — hollow,
supports, the drainage check, auto-orient, and every "clear" button — ran over everything
visible on the plate regardless of it. Selecting one of four copies and pressing Hollow
hollowed all four. There was no way to say "these two", and no way to tell from the
button which it would be.

## Decision

The selection is an ordered set. The last one picked is the primary: what the Transform
inspector edits and what the repair report under the list describes.

A plain click replaces the selection; a click with Ctrl, Cmd or Shift adds to it or takes
that one back out, in the viewport and in the plate list alike. Ctrl+A picks the plate,
Escape drops the selection, Delete removes what is picked.

`Scene::targets` is what a tool acts on: the selection, or everything printable on the
plate when the selection is empty. It filters through what is on the plate being edited
and not hidden, so a stale pick can never reach a model the user is not looking at.
`Scene::scope` puts the same answer into words, and the panels print it under their
buttons rather than each writing their own sentence.

The gizmo takes the whole selection. `transform-gizmo` already accepts a slice of
transforms and puts the handles on their shared pivot, so several models move, turn and
scale as one body.

## Consequences

The surprise is gone in both directions: with nothing picked the buttons still do the
whole plate, which is what they did before and what a plate of one model wants, and the
line under each one says which it will be.

The cost is a rule with two branches. Someone reading `hollow_tasks` sees `targets()` and
has to know that an empty selection means everything — the alternative was making every
user press Ctrl+A before every action.

Undo covers all of it without a line of its own, because the selection lives in `Scene`
and a snapshot is the whole scene (ADR 0086). That also means undo steps through
selection changes, which is more entries on the stack than an editor that keeps the
selection outside the document.

Reopen this if a rubber-band or a box select arrives: dragging in the viewport is
currently the camera, so a band needs a modifier of its own or a tool of its own, and
that is a decision about the input map rather than about the selection.

## Alternatives considered

### Always the selection, never the plate

No branch, no surprise, and the button always means one thing. It makes the common case —
one model on the plate, hollow it — two actions instead of one, forever.

### A "apply to all" checkbox beside each action

Explicit, and it shows the rule in the interface. It is one more control per tool panel,
all of them saying the same thing, and it still has to default to something.

### Keeping the selection outside `Scene`

Would keep undo entries down. Then moving a model and picking it are two different kinds
of state with two different lifetimes, and every job that takes a `&Scene` would need the
selection passed beside it.

### The option that won, and what it costs

`Scene` now answers four questions that all sound alike — `objects`, `here`, `printable`,
`targets` — and the compiler cannot tell which a new call site meant. Only the names
guard it, the same weakness ADR 0098 already admitted for the plate methods.
