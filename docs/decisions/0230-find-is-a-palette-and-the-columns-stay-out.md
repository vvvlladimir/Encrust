# 0230. Find is a palette, the plate column stays out, and a block explains itself on its question mark

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

ADR 0217 drew Find in the top bar and left it doing nothing. The v2 window also kept three
habits the mockup does not have: the plate column folded away to its plate squares when its
edge was dragged past a floor or `\` was pressed, and the inspector had a fold button of its
own; a block turned on by one switch named itself twice, once as its title and once as the
switch; and every tool's action carried a line of explanation under it, even though each
block already reads its explanation out of the question mark beside its title.

## Decision

- **Find** (the top bar's chip, ⌘K) opens a palette over the window: every tool, every
  action of the key table and every model on the plate, matched by every typed word being in
  the name, names starting with it first. Up and Down move along it, Enter does what is lit,
  Esc or a click outside closes it. A tool is opened rather than toggled; a model is picked.
  It is `Action::Find`, so the table, the sheet and the tooltip know it like any other key.
- **The plate column cannot be folded.** Its edge drags between `SCENE_W_RANGE` and nothing
  past it; `\` and the View menu's Plate contents are gone. The inspector loses its fold
  button and still folds when its tool is pressed again.
- **The question mark** is one widget, `help_mark`: a quiet ring, a tooltip on hover, the
  same text in a popup a click holds open, and a button to the keyboard and a screen reader.
- **A switch that turns a whole block or group on stands in its heading**
  (`section_with_switch`, `subheading_with_switch`), not as the block's first row.
- **An action has no caption.** What it does is its tooltip; only the reason it cannot run
  is written under it.
- **Check and Export cut the stack as soon as they are open**, as the Preview mode does,
  rather than asking for the layer view first: both read their figures off it.
- **The file's format is chosen in the printer's settings only**, where the machine's
  container is defined; Export no longer offers it.
- The print chip stands in the middle of the window rather than against Find.

## Consequences

Every action and tool is two keystrokes away without the user knowing its key or its place
on the rail, and a model on a crowded plate is found by name. The palette reads the key
table, so an action added to the table is in it with no further work.

The plate column is always on the stage; a user who wanted the room it takes has to narrow
it instead. Reopen this if a small screen leaves the model too little room.

Opening Check or Export on a large plate starts a cut the user did not press for; it is
the same cut Preview makes, cancelled and redone the same way when the plate changes.

A tool's purpose is no longer visible until it is hovered. The text is not lost, only moved,
and the reason a button is greyed out is still written where it is greyed out.

## Alternatives considered

### Search fields and settings as well as actions

What the button's old tooltip promised. Rejected for now because a field has no name the
window keeps outside the code that draws it; finding one would mean registering every row
of every panel. Actions, tools and models already have names in one place each.

### Keep the fold and only hide its button

Rejected: a drag past the floor still folded the column, so it could be lost without a
visible way back.

### The decision above, and what it costs

The palette is one more modal over the plate, which the viewport has to be told about
(`ViewOptions::modal`), and a user who reads the panel top to bottom no longer sees what the
action at its foot will do before hovering it.
