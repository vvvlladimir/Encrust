# 0339. Retune a shipped automatic preset, never add one

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

An automatic profile only shipped (ADR 0325): the user tuned the shapes it builds to, which
are support profiles saved like any other, but where it places supports and the thresholds
that size them reset with every new project. The Supports card (ADR 0338) shows those numbers
under the automatic preset and marks it edited, so Save must have somewhere to write them.
Letting the user add automatic presets of their own would need a way to make one, name its
shapes and keep them apart from the shipped ones, which nothing asks for.

## Decision

**The user keeps a copy of a shipped automatic profile, under its id, in `auto/` of the
profile directory**, as `Kind::Auto`. `Catalogue::save_auto` writes it and refuses an id that
does not ship. Loading lays a copy only over an automatic profile this build ships: a file
under any other id is ignored. Throwing the copy away brings the shipped one back. An
automatic profile is exported like any profile but never imported.

## Consequences

Save in the card writes the automatic preset at once, with no Save as new. The set of
automatic presets is fixed by the build. A shipped profile that changes later is hidden by an
older user copy until that copy is thrown away, as with any shipped profile.

## Alternatives considered

### Save as new automatic presets

Rejected: a user preset would name shapes by id, and nothing yet lets a user choose them.

### Keep the edits in the project only

Rejected: the numbers a user settles on would be lost on the next plate.

### A copy per shipped id, and what it costs

A sixth `Kind` every match over kinds must answer, even where an automatic profile has no
place, such as an import.
