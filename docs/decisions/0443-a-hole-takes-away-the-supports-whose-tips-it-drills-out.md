# 0443. A hole takes away the supports whose tips it drills out

- **Status:** Accepted
- **Date:** 2026-11-12

## Context

A support point is a contact fixed in the model's own space. A hole drilled where a tip
touches the model takes that surface away, and since ADR 0441 the hole no longer cuts the
tree either, so the tree is grown again to the same contact and stands with its tip in the
hole, holding air. Growth keeps a contact it finds no surface under where it was put
(ADR 0323, 0329), so nothing downstream drops it.

## Decision

A support point, or a tip of a frozen tree, whose contact lies inside a hole drilled into the
model goes: `core_volume::drilled_out` asks the holes, `ModelSupports::clear_contacts` takes
the points and tips, and a frozen tree left with no tip goes. A hole of `HoleKind::Support`,
in the raft, takes nothing.

- The window asks as a hole is drilled, of that hole alone, in the same edit
  (`SceneObject::drill_hole`), so one undo takes back both and a tip set into a hole later
  stays.
- `core_engine::supports_of` asks on opening a project, for the window and the command line
  alike; where a tip went, the saved trees are grown again rather than stood as saved.

## Consequences

A drain under a supported area leaves no tip in it; the tips beside its rim stay. The supports
taken away are not told of; reopen this if a user misses them. A run on the command line places
its points on the drilled surface already.

## Alternatives considered

### Move each such tip onto the rim of its hole

Keeps the count, but the nearest surface is the hole's wall, a face looking sideways that a
head cannot hold, and a tip pushed past the rim lands where a neighbour may already stand.

### Drop a contact that is off the surface wherever it is grown

One rule in `core-supports` for any edit that takes a surface away, but growth keeps a
contact put under a face on purpose (ADR 0323, 0329), and many placements rely on it.

### The option that won, and what it costs

Supports placed by hand go without a word, and only a hole's body is asked: a surface taken
away by any other edit still leaves its tips where they were.
