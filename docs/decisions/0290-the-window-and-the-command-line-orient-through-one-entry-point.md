# 0290. The window and the command line orient through one entry point

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

ADR 0277 has `core-engine` prove an orientation by printing the search's finalists. The
window did; `encrust slice --orient` still called `core_plate::orient`, the search alone, so
the same model on the same printer could be turned one way in the window and another on the
command line. The command line also turned the model about the file's origin and only
dropped it back onto the plate, so a model drawn away from the origin landed elsewhere in x
and y, possibly off the plate.

## Decision

`core_engine::orient_for_print(printed, settings, trial)` is the one entry point. With a
`Trial` it prints the finalists and ranks them by `prove`; with none it answers the search's
finalists in its own order. Each `Choice` carries the pose, its `Printout` where one was
printed, and its `Ruling`. It replaces `orient_proven`; `prove` stays public for
`orient-lab`, which times the two stages apart (ADR 0278).

The window passes a trial when a printer is chosen. The command line passes one when it
knows a printer and the supports the model will stand on, since a trial stands the model on
supports; with no supports asked for, a trial would judge a print that is not made. A turned
model is stood on the plate over the middle of its bounds in x and y.

## Consequences

The two front ends answer the same pose for the same model, printer, resin and supports.
`--orient` with a printer and supports now takes seconds rather than a fraction of one, as
the window's **Auto-orient** does. Reopen if a front end needs a third way to choose.

## Alternatives considered

### Keep `orient_proven` and branch in each front end

Two call sites deciding when to prove is how they drifted apart.

### The option that won, and what it costs

A command-line run with a printer and supports pays for the trials on every `--orient`, and
one with no supports still takes the search's word.
