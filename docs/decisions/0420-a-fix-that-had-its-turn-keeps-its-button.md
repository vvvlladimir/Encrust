# 0420. Keep a fix's button on its card after the fix has had its turn

- **Status:** Proposed
- **Date:** 2026-11-10

## Context

ADR 0416 dropped a fix's button from its card once the fix had answered the kind, so that a
press could not repeat a fix that had done what it could. The answer stands while only fixes
follow it, and Fix all runs several. A later step can change what the first one answered: on
a broken model, supports stood for 21 islands, then the steps after them left 200 islands
on the card with Remove them and no Support them. Nothing on the card could answer them.

## Decision

A card shows every button its kind offers, whether or not a fix has answered it; what the
fix said stays above the buttons. Fix all still counts an answered kind as left to the hand,
so it never repeats a fix. This replaces the clause of ADR 0416 that drops the button.

## Consequences

The user can always answer what a card shows. A press that finds nothing new to do changes
nothing, records no answer and says so in the status line, for example that no island had
an underside to stand a support on. Reopen this if repeated presses that do nothing
become the common case.

## Alternatives considered

### Drop the button only when a press would change nothing

It would answer exactly, but knowing that means running the fix on every frame: finding the
floor of every island or the wall of every cup, which grows with the issues on the card.

### The option that won, and what it costs

A card keeps a button that may do nothing, and the user learns that only by pressing it.
