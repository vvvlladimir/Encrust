# 0389. Save a support profile's form by a press, and ask before leaving it unsaved

- **Status:** Accepted
- **Date:** 2026-10-31

## Context

ADR 0120 dropped Save and Revert for printers and resins, and 0220 and 0233 carried that to
every profile form: each number was written to the user's directory once it settled. The
support profile form has since grown to ten blocks, tabs over them and a reset that rewrites
every number at once. A user working through it could not tell a trial from a decision: each
edit reached the plate's picker and every model using the profile at once, and the header had
to say *Saved as you type* to explain why there was no button.

## Decision

**A support profile's form, by hand or automatic, holds its edits until Save is pressed.**
Save stands at the right of the action bar in the accent, greyed while the form holds nothing
unsaved. A number is marked in amber, with an arrow putting it back, where it leaves the saved
one, so a press of Save clears every mark; the shipped numbers are Reset to defaults' alone. Undo and redo walk the edits as before; Reset to defaults only changes the form.

**Leaving the form with unsaved edits asks first**: going back to the table, turning to
another page, closing the window or following its links to the sheet of keys or Machine and
resin. The question names the profile and offers Save, Discard and Cancel; Esc or a click
outside is Cancel. Where it goes waits in `Settings::leaving` until the answer
(`settings/leaving.rs`). Duplicate and Export, which take the profile as last saved, are
greyed while the form holds unsaved edits.

Printer and resin forms still write as they settle (ADR 0120).

## Consequences

- An edit reaches the plate only when it is meant to, and a whole trial is dropped in one
  press.
- The window now has two models of editing: Save for support profiles, written as it
  settles for machines and resins. The tests in `settings/leaving.rs` hold the first.
- Quitting the app with unsaved edits in the form drops them without asking; the window is
  modal, so that takes quitting while it is open.
- Reopen this if the machine and resin forms are to share the model, or if users lose edits
  to the quit above.

## Alternatives considered

### Keep writing as edits settle, with Save only as a reassurance

No change in what is stored, but a button that does nothing a user can see, beside a
header saying the same.

### Hold edits and drop them silently on leaving

One fewer dialog, but a slip of the back arrow loses a whole session's tuning.

### Held until Save, asked on leaving — what it costs

A modal question on every unsaved exit, five ways out to route through one gate, and two
editing models in one window until the other forms follow.
