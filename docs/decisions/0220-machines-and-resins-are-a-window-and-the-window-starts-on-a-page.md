# 0220. Edit the machines and resins in a window over the plate, and start on a page

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

Since ADR 0052 every profile was edited on the Settings screen, which replaces the plate.
Its Printers page was a tree: the machines, the resins of the open one under it, and one
form beside the tree for whichever node was picked. Two things followed. The machine and the
resin the plate prints on were picked in the top bar's chip, but set up on a screen that
hid the plate; and a printer's resins, which differ by three numbers, could only be compared
by opening them one at a time. Updates and the support profiles share that screen and have
neither problem.

An empty plate showed one card in the viewport, "No model on the plate", with one button.
A first run has a machine to choose before a model means anything, and a returning user
reopens what they had last; the card offered neither.

## Decision

**Machine and resin** is a modal window over the plate, opened from the top bar's chip, the
File menu, the Settings screen and the start page. The machines the user installed are
down its left, each with its maker and container, a glyph when it takes a file over the
network and a check on the one the plate prints on; **Add a machine** under them puts the
library (ADR 0158) in the tabs' place. The machine picked has three tabs:

- **Resins**: the resins set up on it as a table of name, type, layer, exposure and bottom
  exposure as this machine has them, narrowed by type. A double click or **Edit** opens the
  resin's form in the table's place. **Duplicate**, **Take off this machine** and **Add a
  resin** keep their meaning from ADR 0120, 0196 and 0197. **Use this resin** puts it, and
  its machine, under the plate.
- **Machine**: the profile's form, **Remove this machine** and **Use this machine**.
- **Network**: what it takes a file over, from ADR 0156.

The window holds the keyboard, and Esc takes down the question or the calculator over it
first; both become modals of their own so they stand over the window. The Settings screen
keeps Supports and Updates, laid out as blocks — a title in a column of its own beside its
fields — with **Machine and resin** and **Shortcuts** under its pages.

The window **starts on a page** under the top bar: a zone a file is dropped on, the machine
the plate prints on or a warning that there is none, and the five files opened or saved
latest. It is left for good the first time a model, a project or a sliced file arrives or a
tool is picked. The files are kept, newest first and eight at most, in `preferences.json`
beside the rest (`recent.rs`); a browser hands over bytes and keeps none. One that has
gone is dropped from the list when it is clicked.

## Consequences

- The plate stays in view while its machine or resin changes, and the resins of a machine
  are compared on one screen.
- Nothing about a profile, the catalogue or what is written changed: the forms, the
  autosave and every deletion's question are the ones the Printers page had.
- A resin in the pool is still deleted from the **Add a resin** menu. A filter by maker, as
  the design draws it, needs a field resin profiles do not have; reopen if one is added.
- The start page is shown once a session. An emptied plate falls back to the viewport's
  card; reopen if users ask for the page again.

## Alternatives considered

### A picking window, with editing left on the Settings screen

Less to move, but it splits one machine between two places and still hides the plate to
change a number.

### The start page whenever the plate is empty

Removing the last model would throw the rail, the inspector and the plate column out of the
window under the user's hand.

### The option that won, and what it costs

A modal is fixed at most 1080 by 720 points, so the resin form is narrower than it was on
the screen, and a second modal over it is the only way to ask anything there. The recent
list writes paths into `preferences.json`, which now names the user's files on disk.
