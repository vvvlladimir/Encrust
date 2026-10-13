# 0247. Make a profile chosen from the list a group of its own

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

A support carries a group and a group carries a profile (ADR 0094), and the panel's list of
saved profiles wrote the one chosen over the profile of the group in hand. So choosing
Heavy after placing Light rebuilt every Light support as Heavy: two kinds of support on one
part meant making a group first, by a button in another tool, and then choosing its profile
— two lists to keep in step. ADR 0246 kept a chosen profile on the picked models alone,
which still rebuilt everything standing under them.

## Decision

Choosing a profile from the list chooses the group built to it — the one whose profile has
that name — and makes one, named for the profile, when there is none. What stands stays in
the group it stands in. The list is on the Shape, Paint and Supports tools alike.

- A group nothing stands in is retuned to the chosen profile instead of being left behind
  empty, and renamed with it unless the user had named it.
- The profile in hand chosen again goes back to what is saved, on the picked models too.
- A support changes kind by being picked in Edit and moved to the group in hand (ADR 0095).
- Editing a number still rebuilds what is built to the profile in hand, on the picked
  models (ADR 0246); the panel says how many supports that is.

## Consequences

Placing thin supports on a face and thick ones under a chin is choose, place, choose,
place. The groups list is now mostly a readout of the kinds on the plate, with their
counts and the way to drop one. Two groups cannot be built to one saved profile through
the list; **New group** still makes a second by hand. Merging stays per group, so two kinds
share no trunks (ADR 0094).

## Alternatives considered

### A snapshot of the profile on every support

The simplest rule to explain, but a standing support could then only be retuned by
picking it, trunks could not be shared across snapshots, and the project file would hold
as many profiles as there were edits.

### Leaving the list alone and warning how many supports it rebuilds

Keeps two lists the user has to work in step, which is what was hard to begin with.

### The option that won, and what it costs

Browsing profiles with supports standing leaves a group behind for each one that was
placed with, and retuning what stands to another saved profile is now two steps.
