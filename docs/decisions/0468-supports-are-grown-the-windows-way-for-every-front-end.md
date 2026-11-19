# 0468. Supports are grown the window's way for every front end

- **Status:** Proposed
- **Date:** 2026-11-19

## Context

ADR 0466 makes every action one edit carried out by one `apply`. Supports are two: the window
grows them by its own job (`encrust-app/src/job/place.rs`): the model sliced where it stands,
`generate_supports` fed the painted and blocked faces, the side meant to be seen, the user's
seeds and the model's low points, then `strengthen`, then the trees grown and kept on the
model. `--supports`, the autopilot (ADR 0402) and the recipe steps go through
`core_engine::stand_under`, which calls the same two functions with what the command line
has: no paint, no seeds, a lift of its own. A `.encrust` keeps only grown trees, which
`open_plate` reads and never decides again, so the two ways cannot both write a plate the
other opens alike.

## Decision

The window's way is the one way. Its job's steps — the needs from the slices, fed what the
window gives (painted, blocked and shown faces, seeds) and nothing where a front end has none,
then strengthened — move into `core-engine` beside `stand_under`, which `core-document` already
reaches and the autopilot lives in. The window's job calls them on its worker; `stand_under`,
`stand_seated` and `stand_at` become calls of them, and `Edit::Supports` in `core-document`
runs them with the trees grown as the window grows them, so `--supports`, the autopilot, the
recipes and a client grow the trees the window grows and a plate file keeps them alike.

## Consequences

A model supported on the command line, by a chat client or in the window prints the same
supports and opens the same in each. The command line's supports change where its lift or its
lack of paint differed from the window's; `support-lab`'s baselines are run again and kept
anew. The window's job shrinks to its worker and its landing. Reopen if a front end needs a
support the window cannot place.

## Alternatives considered

### The engine's way for both

`stand_under` is what the autopilot and its benches were tuned on, but the window would then
lose its paint, its seeds and the trees users see grown, or keep them beside a second way.

### Leave the two ways

Nothing moves, but a client's supports never become an edit, and a plate supported in one
front end is supported otherwise in the other.

### The option that won, and what it costs

The largest move of ADR 0466's: the window's support job and its tests, and every caller of
`stand_under`, with the support bench run again to show what the command line now prints.
