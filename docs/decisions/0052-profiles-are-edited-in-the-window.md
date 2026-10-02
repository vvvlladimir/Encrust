# 0052. Edit profiles in the window, into the user's own directory

- **Status:** Accepted
- **Date:** 2026-09-20

## Context

ADR 0049 gave the slicer a catalogue and a user directory that overrides it by id, and
ADR 0050 made a resin's exposure a property of the resin *and* the machine. Both left the
same gap: the only way to correct a number is to write TOML by hand, into a directory the
user has to find first. Every number in the shipped catalogue is transcribed rather than
measured, so correcting one is not an edge case — it is what the first exposure test
produces.

The window already had two pickers, and a Slicing panel full of numbers that were
read-only copies of whatever profile was loaded. Editing a machine there would have been
wrong: the inspector is about the plate in front of the user, and a build volume is not.

## Decision

`encrust-app` gains a Settings screen. When it is open the rail, the inspector and the
stage are not drawn; everything between the title and status strips is a list of sections
down the left and the form for the open section beside it. Two sections exist now,
Printer and Resin, and the shape is chosen so that print settings and support settings
move there later.

Saving writes a TOML file into the user's profile directory under the id in the form's Id
field and takes it into the catalogue in memory, so the change is live without a restart.
Editing a shipped profile and saving it keeps the user's copy over the bundled one, by
ADR 0049's rule; changing the id first makes a copy instead. "Delete my copy" removes the
file and brings the shipped profile back, or drops the id when nothing was shipped under
it. Nothing is ever written into `assets/profiles`, which stays the shipped catalogue.

The resin form edits the numbers *as the machine in hand needs them*, not the file. On
save, what the user changed is folded back into that machine's `[printers.<id>]` table by
`PrinterTuning::of_changes`, and every other machine's table survives untouched. A switch
turns that off, and then the numbers become the resin's own, for every machine with
nothing of its own. The name and the density are never a machine's business and always
land on the resin itself.

`PrinterProfile::save` and `MaterialProfile::save` validate before writing, so a profile
that would not load is never written. A `Catalogue` knows the directory it writes into,
which is set when a directory is laid over it; a bundled-only catalogue has nowhere to
save and says so rather than guessing.

## Consequences

A user can describe a machine nobody has written a profile for, measure their resin on it
and keep both, without leaving the window or knowing what TOML is. The catalogue, the two
pickers and the CLI all see the result, because it is a file in the same directory the
`--printer` id resolves through.

The window now holds a profile twice while the screen is open: the catalogue's copy and
the form's draft. They are reconciled only on Save or Revert, which is what makes Revert
possible, and what makes an unsaved edit disappear when the screen is closed.

Saving the machine that is in use puts the plate back under it, so a changed build volume
moves the models standing on it. That is a visible jump, and it is deliberate: the plate
is what the numbers describe.

`printer-profiles` now writes files as well as reading them. If profiles ever need to be
edited from more than one place at once, the directory becomes shared mutable state and
this gets revisited.

## Alternatives considered

### A modal dialog over the plate

Less code and no new arrangement to maintain. Rejected because the sections are going to
keep arriving — print settings, support settings, machine limits — and a dialog that grows
into a second application is worse than a screen that was one from the start.

### Editing in the inspector, beside the plate

The numbers are already there to read. Rejected because the inspector is about the
selected object, its scroll is already long, and a build volume edited by accident while
placing a model is a bad afternoon.

### The option that won, and what it costs

A screen that replaces the plate cannot be used while looking at the model, so tuning an
exposure against what the preview shows means going back and forth. The draft-and-save
model also means an edit is lost if the screen is closed without saving, with no prompt —
the status bar says what was saved, and nothing says what was not.
