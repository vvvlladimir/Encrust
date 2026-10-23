# 0335. Stand trunks on the nodes of a square grid when a profile asks for it

- **Status:** Accepted
- **Date:** 2026-10-23

## Context

Branching merges tips wherever their struts meet (ADR 0040), so trunks stand at irregular
places: the forest is economical but uneven to brace, and cutting it away means hunting for
each foot. A grid of trunks the same distance apart is the other common way to build a
forest in resin: every trunk has neighbours at a known distance, the braces come out
regular, and the supports come off in rows. It is a property of how a kind of support is
built, so it belongs with the rest of a profile's shape.

## Decision

- **A section of the profile.** `[grid]` holds `enabled` and `spacing_mm`, 5 mm unless set,
  1 to 15 mm; a profile without it has no grid, so every profile and project already saved
  reads as before.
- **Snapped, not merged.** Where every group of a family has the grid on, `grow` stands
  each tip on a node of a square grid anchored to the plate in place of merging: the
  nearest node within two rings whose 45 degree strut from the neck clears the model and
  under which a trunk lands straight down. The tips of one node hang one above another on
  one trunk over the node, the lowest joint standing it. A tip no node takes stands as it
  would without the grid. Such trunks are not leant towards a neighbour (ADR 0240): they
  stay on their nodes, and the bracing graph ties neighbouring nodes as it ties any trunks.

## Consequences

A gridded forest is regular to brace and to cut away, and a thinner group may still run its
struts into its trunks (ADR 0248). It stands more trunks than a merged forest, and every tip
leans up to two spacings to its node, so a wide spacing makes long struts. Reopen if the
struts or the count are reported.

## Alternatives considered

### A switch on the model or the plate

One grid for every group on a model, but a different section of the file, and a group of
small pillars could not stay off it.

### Snap the merged trunks afterwards

Moves trunks the merge had already cleared of the model, so each move needs the checks
again, and two trunks may end on one node.

### The option that won, and what it costs

A family switches between two ways of growing, and a grid of groups of different spacings
takes its first group's.
