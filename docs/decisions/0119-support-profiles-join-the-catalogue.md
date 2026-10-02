# 0119. Support profiles join the catalogue; a group tunes a copy

- **Status:** Accepted
- **Date:** 2026-09-28

## Context

Support shapes were three compiled-in presets plus a file dialog in the Supports panel.
There was no list of the user's own profiles to pick from, edit, or delete, while printers
and resins already had one: the catalogue with a user directory over it (ADR 0049).

## Decision

`Catalogue` carries a third kind, `Kind::Support`, bundled from `assets/profiles/supports/`
and overlaid from `<profiles>/supports/`, saved and forgotten like the other two. The
Settings screen edits them. A support group on the plate picks one and keeps its own copy,
tuned in the Supports tool; the tool says when that copy no longer matches any profile, and
offers to save it as one. Saving a profile never changes a group already on the plate.

## Consequences

One list of support profiles, the same files-on-disk model as printers and resins. A group
remembers numbers, not an id, so which profile it came from is found by comparing values,
and a group edited back to a profile's numbers reads as that profile again.

## Alternatives considered

### A group that follows its profile by id

Editing the profile would retune every group built to it. Lost because a plate would then
change under the user when a profile is edited for another print, and a project would
depend on files outside it.

### Keep the file dialog in the panel

Nothing to list or delete, and no place for more than a handful of fields. Lost to the
request for a profile manager.

### The option that won, and what it costs

A group's link to its profile is by value only: two profiles with identical numbers are
indistinguishable, and renaming a profile does not rename what the group shows.
