# 0226. Keep a tool's settings as named presets in the profile catalogue

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

Support shapes are kept as named profiles in the catalogue (ADR 0049), written through the
`ProfileStore` that is a directory at the desk and the page's storage in a browser
(ADR 0181). The Hollow and Drain tools had no such thing: a wall and infill that work, or
a hole size, had to be typed again on every plate. The mockup gives both tools a Profile
row (ADR 0218). Their settings are `core_engine::project::{HollowState, DrainState}`, built
of `core-volume` types, and `printer-profiles` depends on nothing in the workspace.

## Decision

`printer-profiles` gains `Kind::Hollow` and `Kind::Drain`, kept under `hollow/` and
`drain/`, and a `Preset { name, settings: toml::Table }`. The catalogue lists, saves and
forgets presets by kind; `Preset::of` and `Preset::read` turn any serde type into the table
and back. The crate never learns the shape of a tool's settings: the app saves its own
`HollowState` and `DrainState`, and a preset that no longer reads as one is skipped.

Nothing ships: every preset is the user's. The panel shows the preset the numbers match,
"Unsaved" when they match none, and a name field to keep them.

## Consequences

Any later tool can keep presets with one more `Kind` and no new type in the crate. Because
the table is opaque to it, a preset written by an older release whose settings type has
since changed is only found out when read, and is then left out of the list rather than
loaded wrong. Presets travel in the profile directory, not in a project.

## Alternatives considered

### Typed hollow and drain profiles in `printer-profiles`

The crate would have to mirror `HollowMode`, `InfillPattern` and the rest, a second copy
of each to keep in step.

### A store of the app's own, beside the preferences

A second place for a user's files, and a second way of reaching the browser's storage.

### An opaque table, and what it costs

The catalogue cannot validate a preset when it loads it, as it validates a printer.
