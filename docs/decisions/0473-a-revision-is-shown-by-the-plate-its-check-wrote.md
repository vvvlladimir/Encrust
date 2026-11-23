# 0473. A revision is shown by the plate its check wrote

- **Status:** Accepted
- **Date:** 2026-11-23

## Context

ADR 0459 shows a chat client's revision in the open window by running its recipe there:
the window stages the model a second time, and what no recipe step says — points by hand, a
picked pose, the modelling, walls kept solid — is left out and listed as `unshown`. Since ADR
0472 the command line stages every model as a `Document`, and `--project-out` writes the plate
a run leaves; a revision made of edits on the window's plate already keeps it as `N.encrust`
(ADR 0466). The window opens a project only in place of its whole plate, its undo with it.

## Decision

- **Every check writes its plate.** The server passes `--project-out` on each check of a model
  and keeps the file as `N.encrust`; the command line writes it after a recipe or `--prepare`
  too, and refuses only `--edits` with one.
- **The window is handed that file.** Link schema 4 carries `plate`, its path, in place of
  the recipe; the window's link thread reads it, so a bad file is refused with why and the
  drawing thread reads nothing.
- **The window stands it beside what stands.** Once no job runs on the plate it takes off the
  objects the job's last revision stood as and puts the file's objects on its active plate
  (`project::inserted::insert`): an id the plate has taken gets the next free one, and each
  support group is joined to the window's group of its name, a new one where there is none,
  the file's profile kept as the object's own where the window's differs. One undo entry.
- **A revision made on the window's plate is not shown again**: it is there already, and its
  file holds the user's other models too.
- `unshown` and the window's replay of a recipe go; `N.recipe.toml` stays for `save_recipe`.

## Consequences

The window shows what the command line checked, every step and point of it, and is no longer
a second stager whose plate could differ. The job's model is the revision's one object that
is no plug; a revision that leaves pieces has none, so its next one goes by the command line.
A revision's file costs a write of its meshes on every check. Reopen if a file grows past
what a check can write in time, or the window gains steps the command line lacks.

## Alternatives considered

### Open the file as a project

Reuses File, Open project, but replaces the user's whole plate and drops their undo history
with every revision.

### Keep running the recipe

No file per check, but the window stays a second stager and `unshown` never empties.

### The option that won, and what it costs

A new way into the scene beside opening a project, with support groups matched by name: two
groups of one name and different numbers are told apart only by the object's own profile.
