# 0257. Record what an edit sets off as part of it, and do it again after an undo

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

The window changes the plate by itself after some edits: a model with supports stands on
its raft, so taking every support off lowers it by the raft's thickness, and switching
the raft on raises every supported model (ADR 0238, 0253). That settling ran at the start
of the next frame, after the history had already recorded the edit (ADR 0086), so it
landed as an edit of its own. `Cmd+Z` then put back the half-way plate, the next frame
settled it again and recorded that over the redo: after *Remove all* on a rafted model
the supports came back for one frame and went again, every time.

Switching the raft on changes a tool value and the plate in one frame, and ADR 0192 makes
those two entries, so taking it back took two presses with the same loop between them.

## Decision

- **The window settles the plate before it records the frame.** What an edit sets off is
  in the same entry as the edit.
- **An undo or a redo settles the plate it put back**, and measures the next edit from
  the settled plate, so nothing the window does by itself is recorded over the redo.
- **A frame that changed the plate and a tool value is one entry**, which puts both back.
  This supersedes ADR 0192's rule that a tool value is always an entry of its own.

## Consequences

Every follow-up the window makes by itself has to run in `SlicerApp::record` and in
`History::follow_up`; settling onto a raft is the only one today. One left out comes back
as this loop.

A slider released in the very frame a background run lands its result is now one entry
with it, and `Cmd+Z` takes both back. Reopen if that is seen in practice: the answer is to
record a landing run in a frame of its own.

## Alternatives considered

### Keep the raft lift out of the transform

The settling would then be no edit at all. Rejected because the transform is what every
consumer reads the model's height from, from the viewport to the sliced file.

### Leave the entries apart and only settle after an undo

Ends the loop. Rejected because switching the raft on would still need two presses to
take back, the first of which shows nothing.

### The option that won, and what it costs

A tool value and a plate edit that only happened to share a frame are taken back together.
