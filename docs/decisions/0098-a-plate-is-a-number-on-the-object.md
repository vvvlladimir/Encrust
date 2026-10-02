# 0098. A plate is a number on the object, not a scene of its own

- **Status:** Accepted
- **Date:** 2026-09-26

## Context

A project has to hold several build plates: a model too big for one, or a batch of parts
split across runs. The window already holds one `Scene`, and the undo stack (ADR 0086)
snapshots that whole `Scene` on every edit, sixty-four deep.

Nearly every tool in the window reads "everything on the plate", and almost all of them
already did it the same way: iterate the objects and keep the visible ones. Slicing is
the one that cannot use "the plate in front of the user", because a run over the project
cuts plates nobody is looking at.

## Decision

One `Scene` holds every model in the project. Each carries `plate: u32`, and the scene
carries the plate names and which one is being edited.

`Scene::here` is what is in front of the user, `Scene::printable(plate)` is what slicing
a given plate would take, and `Scene::objects` stays the whole project — used only where
a change is project-wide, such as removing a support group. Every tool, the renderer and
picking moved to the first two.

Slicing takes a plate number. A run over the project queues the plates that have anything
on them and cuts them one after another, one job at a time, each into the typed name with
the plate's own name appended.

## Consequences

An undo snapshot costs the same with six plates as with one, and undo covers adding,
renaming, removing and moving between plates without a line of its own.

The cost is that "every object" and "the objects here" are now two different things
spelled a line apart, and a new call site that reaches for `objects()` when it meant
`here()` will silently act on plates the user cannot see. The three methods are named for
what they answer rather than for the field they filter on, which is the only guard there
is.

A plate's name is a label, not a key: two may read the same, because the user types them.
What is unique is the number, so the widget drawing the tabs keys on position and a run
over the project falls back to the number when two plates would claim one file name.

Cutting the whole project is serial. The cores go into the layers of one plate, which is
where the parallelism already is; two plates at once would halve each other.

Reopen this if a plate ever needs settings of its own — a different printer per plate, or
its own exposure. Then a plate stops being a number and becomes a record, and the scene
splits.

## Alternatives considered

### A `Vec<Scene>`, one per plate

The obvious shape, and it makes "the objects here" impossible to get wrong. It multiplies
the undo stack by the number of plates, because a snapshot is a whole scene: six plates
would mean six times the placements kept, for an edit that touched one of them.

### A plate as a filter on the existing `visible` flag

No new field at all: an object not on the plate is simply hidden. It collides with the
visibility the user controls — hiding a model and switching plates would then be the same
state, and coming back would show what had been hidden.

### The option that won, and what it costs

`objects()` still exists and still returns everything, so the compiler cannot tell a
caller that meant one plate from one that meant the project. Twenty-odd call sites were
moved over by reading them; the twenty-first, added later, has nothing catching it.
