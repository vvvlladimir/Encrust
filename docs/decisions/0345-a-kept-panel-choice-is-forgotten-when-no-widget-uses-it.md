# 0345. Forget a kept panel choice that no widget has used for twenty runs

- **Status:** Accepted
- **Date:** 2026-10-25

## Context

ADR 0334 keeps a widget's own choices, such as a section's fold or a panel's tab, in
`preferences.json` by egui id. An id is a hash of where the widget stands. A widget that
moves, a title that changes with the language, or a new egui that hashes differently leaves
its old entry behind, and nothing ever removes it. The file grows with every layout change
for as long as the user keeps it.

## Decision

`ui::kept` counts runs. Each choice records the run in which a widget last read or set it.
When the preferences are written, a choice nobody used in the last twenty runs is dropped.
A file written before runs were counted is still read: its choices count as used in run 0.

## Consequences

The kept map holds what the current layout uses, plus what a reworked layout left behind,
for twenty runs at most. A panel the user has not opened for twenty runs opens on its
defaults. Reading a choice now marks it as used, so the preferences are written once more
early in each run in which a widget is shown.

## Alternatives considered

### Keep only the ids read in this run

Simpler, but a panel the user did not open this time would lose its folds at once.

### A cap on the number of entries

This needs no counter. But a cap drops by age with nothing to tell an old entry from a live
one, so a busy layout could lose a fold the user still relies on.

### The option that won, and what it costs

Every entry in the file carries a run number, and the file format changes, with a reader
kept for the old map. A choice that is used less than once in twenty runs is forgotten.
