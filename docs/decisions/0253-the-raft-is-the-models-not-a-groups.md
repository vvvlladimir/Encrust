# 0253. Make the raft the model's and the plate's, not a group's

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

The raft was built from the profile of group 0 (ADR 0094), and since a model could keep a
profile of its own (ADR 0246), from that model's own profile of group 0. The Raft tab sits
in every profile, so with a second group in hand its numbers did nothing, and dropping or
retuning the first group changed the raft under everything. A raft is one slab under a
part: it does not belong to one kind of support standing on it.

## Decision

The raft is a setting of its own, `Raft`, kept in two places: the Supports tool holds the
plate's, and a model may keep one in `OwnSettings::raft`. A model stands on its own where
it has one and on the plate's otherwise.

- The Raft tab of the Shape tool edits the picked models' raft, and the plate's with
  nothing picked, whichever group is in hand. A raft set back to the plate's is no longer
  the model's own.
- The raft a saved profile carries is a starting point: **Use the raft of** the profile in
  hand copies it. Choosing, retuning or dropping a group leaves the raft alone.
- The core still builds the raft from the first profile of the table it is handed, so the
  table built for a model (`with_own`, `ObjectState::support_table`) has that model's raft
  written there.
- The project writes the plate's as `supports.raft` and a model's as `own_raft`, both left
  out when absent. A file without `supports.raft` reads it from its first group's profile,
  which is where it was, so the manifest stays version 4.

## Consequences

One raft per model, edited in one place, unchanged by what happens to the groups. The
raft in a saved profile no longer follows the profile onto a part by itself, which is one
press more for whoever relied on that. `SupportProfile` still has `[raft]`, read by the
command line, which builds from one profile, and as the default a new plate starts from.

## Alternatives considered

### Keeping the raft with the first group and saying so in the other groups' tabs

Least to change, and the raft would still move when the first group is dropped.

### A raft per group

Slabs of different thickness under one part, which is lifted by one of them.

### The option that won, and what it costs

A profile now holds a section that the window does not build from directly, and the
difference between the plate's raft and a model's is told only by a line under the tab.
