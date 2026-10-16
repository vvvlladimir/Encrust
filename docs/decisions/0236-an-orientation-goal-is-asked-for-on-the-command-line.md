# 0236. Ask for an orientation goal and a tilt limit on the command line

- **Status:** Accepted; that the window orients for the default goal alone is superseded by 0282, the goals the command line names by 0284
- **Date:** 2026-10-12

## Context

ADR 0222 searches an orientation for a goal within a tilt and offers both in the window's
Orient section. Folding orientation into the Position tool took that section away: the
window now orients for the default goal with no limit, and nothing reached
`OrientGoal` or `max_tilt_deg` any longer, while the command line's `--orient` always ran
the defaults.

## Decision

The window orients with the defaults alone. The command line takes `--orient-goal` —
`fewest-supports`, `least-peel`, `shortest-print`, `clean-top` — and `--max-tilt DEG`,
each requiring `--orient`, and passes them to `OrientSettings` as ADR 0222 defines them.

## Consequences

The search keeps every goal ADR 0222 tuned, and a user who wants one has it from a script
or a batch. The window and the command line differ here: a file sliced in the window is
oriented as `fewest-supports` with no limit. A user asking for a goal in the window is the
signal to put the control back in Position.

## Alternatives considered

### Put the controls back in the window

Undoes the folding of Position, which made one short section of what were two.

### Delete the goals

Throws away tuned work that costs nothing to keep, and the command line would lose a choice
it never had a reason to lack.

### Offer them on the command line only, and what it costs

The two front ends no longer orient alike when a goal is asked for, and the window's users
cannot reach a goal at all.
