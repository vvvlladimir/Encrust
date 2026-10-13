# 0245. Fill an overhang with its colour instead of hatching it

- **Status:** Accepted
- **Date:** 2026-10-13

## Context

ADR 0219 drew both an overhang and whatever stands past the build volume as diagonal
stripes 3.4 mm apart. On an overhang the mark is the thing a support is aimed at: a tip
is put inside the marked patch, a head's radius in from where it ends. A patch that ends
in a row of stripe ends has no edge to aim at, and a ledge narrower than the pitch reads
as one stripe or as none.

## Decision

A surface leaning past the overhang angle takes the overhang colour at 0.85 over all of
it. The hatch stays for what stands past the build volume, which is a warning about a
whole part rather than a patch with an edge that matters.

## Consequences

Where the need for a support ends is one line on the model. A filled overhang and a
filled danger colour elsewhere now look more alike; the overhang colour is its own token,
and the two are never on the same tool. Reopen this if an overhang has to be told from a
second filled mark on the same face.

## Alternatives considered

### A finer hatch

Stripes closer together give a patch a clearer edge, but they even out to a flat half
tone as soon as they are nearer than a pixel, which is most views of a whole plate.

### The option that won, and what it costs

A filled patch hides the shading of the face under it, so the shape of a large ceiling is
harder to read while the Supports tool is in hand.
