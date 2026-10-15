# 0282. Ask which goal to orient for in the window

- **Status:** Accepted
- **Date:** 2026-10-15

## Context

ADR 0236 left the window orienting for the fewest supports alone, and named a user asking
for a goal in the window as the signal to put the control back in Position. That signal
came once the least peel was worth choosing (ADR 0281): a plate that tears off the film
lying flat is one the fewest supports leaves lying flat.

## Decision

Position shows a picker of `OrientGoal::ALL` above Auto-orient, with one line saying what
the chosen goal does. It sets `OrientTool::settings.goal`, which Auto-orient and the title
bar's Auto-orient plate both run with. The goal lasts the session; it is not part of
`ToolSettings`, so neither a project nor the window's remembered settings carry it. The
tilt limit stays on the command line.

## Consequences

The window reaches every goal the command line does, and the default is still the fewest
supports. Reopening the window brings that default back; a user who wants the last goal
remembered is the signal to add it to `ToolSettings`, which changes what a project writes.

## Alternatives considered

### Remember the goal in `ToolSettings`

It would follow a project and the window between runs, but changes the project file for a
choice that is made per model as often as per plate.

### A segmented control of four

Shows every goal at once but takes a row of labels too long for the panel's width.

### The option that won, and what it costs

One more row in Position, and a choice forgotten when the window closes.
