# 0407. Wait at a recipe's pause or failed step with the plate given back to the user

- **Status:** Proposed
- **Date:** 2026-11-06

## Context

A recipe run in the window (ADR 0402, 0403) held the plate and its undo from start to end.
A `pause` step waited for Continue with every tool held, so a user could look but not
touch; Cancel put the plate back. A step that failed with `on_fail = "stop"`, or a gate
that held, ended the run at once and put the plate back as the run found it, throwing away
every step before it. The failures a recipe meets are often ones a hand mends in a minute:
a support profile missing, a hole a gate asks to drill by hand, a model a check finds
floating. Starting the whole run again after the mend repeats every step before it.

## Decision

In the window a pause, a step that fails and does not skip, and a gate that holds make the
run wait, one state with two reasons (`prepare::Wait`). Waiting, the run releases its hold:
what it did so far is one undo entry, and the plate, the tools and undo are the user's.
Continue takes the hold again and goes on, past a pause, or trying the failed step again:
the batch that failed and those after it, or, where the step failed before any batch
started, its decision taken again on the plate as the user left it. End finishes the run
with the plate as it stands. A pause stops wherever it stands, the steps before it skipped
or not: the box names the recipe's step it waits at, so a run stopped at once reads as one. Cancel while a step runs puts the plate back as the run, or
its last Continue, took it.

The Prepare row says it: why in the picker's box, the pause glyph in the accent or the
warning glyph in `danger`, the share done kept, then Continue (the play glyph) and End (the
stop glyph). Preview, which hides the row, shows a notice with Continue and End instead.

The command line is unchanged: it cannot wait on a hand, so a failed step still ends its
run and a pause still says its message and goes on.

## Consequences

A failed recipe costs the user the mend, not the run. A run may now last across edits of
the user's own and be undone in pieces: the steps before the wait, each edit made while it
waits, then the steps after it. Continue tries a step on a plate the run did not leave, so
a step must take the plate as it finds it, as each already does. A wait left alone holds
nothing: another plate or project opened drops it. The window and the command line part on
`on_fail = "stop"`; a recipe reads the same, but its failure is mended by hand in one and
ends the run in the other.

## Alternatives considered

### A failed step ends the run and puts the plate back, as before

One undo for one run and nothing to explain, but every step before the failure is thrown
away for a fault a hand could mend, and the run must start over.

### A pause and a failure as two states

Kept the pause as it was, holding the plate, and the failure free; two kinds of waiting
that look alike in the row and answer Continue apart, and a pause that could only be looked
at.

### One wait with the plate free, and what it costs

The run is no longer one undo once it has waited, and Cancel no longer means the plate as
before the run once Continue was pressed: it means as before the last Continue.
