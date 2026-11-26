# 0483. Edits handed to the window land whole or not at all

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

By ADR 0466 the server hands the open window a revision's edits, which run one after another as
the tools' own jobs, each an undo entry. One that did not land stopped the rest, but those that
had landed stayed on the plate, and the server kept the revision all the same: the window held
half of it and the job all of it. The link thread also took one connection at a time and waited
up to 10 min for edits to land, so another job's line waited past the server's 15 s and was told
there was no window. A revision to show had its plate read on that thread first: a model of
1.4 million faces in a debug window took longer than the 15 s.

## Decision

A run of edits handed over is held for one undo entry (`History::hold`, as Prepare's). All
landed, it is released and answered `applied`. One that does not land puts the plate back as the
edits found it (`History::abandon`) and is answered with why; the server then keeps no revision
built of them, and a revision its flags checked before the hand-over is shown by its own plate
instead (`then_shown`). A plate the user changes while edits land is left as they left it, one
entry, answered with why, and nothing is kept. Prepare holding the history refuses edits.

The window answers each connection on a thread of its own, at most eight at once. A revision to
show is answered once the window takes it, its plate read on a thread aside and stood once read.

## Consequences

The window's plate and the job's latest revision never disagree after a refusal, and an agent's
step is one undo. A client may lose a step the window could not land and must ask again; the
answer says why. A plate that does not read is told on the status line, not to the server.
Reopen if a step's edits should land in part by design, or if eight connections are too few.

## Alternatives considered

### Keep the revision and leave the window as it stands

Rejected: the next revision is built on a plate the revision does not describe.

### Only not keep the revision

Rejected: the window still holds the edits that landed, which no revision describes.

### Raise the server's wait for a revision

Rejected: a client waits on every call while one slow line holds up the link.

### The option that won, and what it costs

A thread per line, an undo entry held across frames by the link as by Prepare, and a step the
client loses when one tool fails where the command line succeeded.
