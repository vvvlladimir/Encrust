# 0421. Write nothing under a primary button

- **Status:** Accepted
- **Date:** 2026-11-10

## Context

ADR 0230 took the caption off every action but kept one line under it: the reason it cannot
run. The plate card then also wrote there that a busy printer gets the file saved instead.
That line comes and goes with the printer's state, so the card's foot jumps while the user
aims at the button, and on a narrow card it wraps to two lines beside a figure grid.

## Decision

- Nothing is printed under a primary button, the plate card's or a tool's foot.
- Why it cannot run, or where its result goes instead, is its tooltip; a greyed-out button
  still answers a hover.
- This supersedes the reason-under-it clause of 0230; the rest of 0230 stands.

## Consequences

A card's foot stays still whatever the printer or the plate is doing. A greyed-out button
no longer says why until it is hovered; the state that blocks it is still shown where it
lives — the printer row's status, the models card's empty plate, Check's issues.

Reopen this if users press a greyed-out primary and do not find why.

## Alternatives considered

### Keep the reason, drop only the plate card's saved-instead line

Smaller, but two rules for one kind of control: the tool feet would still jump as a blocker
comes and goes.

### The decision above, and what it costs

A reason is one hover away rather than read at a glance, and a touch screen has no hover.
