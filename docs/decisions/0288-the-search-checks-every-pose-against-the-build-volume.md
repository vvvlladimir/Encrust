# 0288. The search checks every pose against the build volume

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

Only the proof checked a pose against the printer's build volume (`trial::overflows`). The
search picked its finalists without knowing the printer, so on a large model every
finalist could overflow, and the proof could only rank them by how they failed. The
proof's check compared the two plan widths with the plate's, sorted, with no turn. A part
that fits only at a slant across the plate counted as too big. In the window, a model
turned by auto-orient pivoted about its own origin. It landed away from where it stood,
and often off the plate.

## Decision

- **`OrientSettings::build_volume_mm`** carries the volume as three numbers, or `None` for
  no limit. `core-plate` takes no dependency on `printer-profiles` for it.
- **Every first-pass candidate is checked against the volume.** The check is cheap because
  it reads an envelope, not the mesh: the farthest vertex along each of 256 Fibonacci
  directions, built once per model. Its height counts the lift onto supports. Its plan
  counts a raft's 2 mm margin on every side for a pose on supports. Both are read a
  hundredth of the model's radius larger, because the envelope lies inside the hull by up
  to that much.
- **The turn about the vertical is exact.** `core_geometry::fitting_turn` takes the
  rotating-calipers events of the plan's convex hull. Between two events, the turned width
  and depth are sinusoids, and it solves where each one meets its wall. A pose fits if
  any turn fits. Where none is needed, it stays unturned.
- **A pose that fits no way is fatal** in `Readings::fatal`, which already held the
  proof's overflow. The shortlist takes fitting candidates first on each reading. A pose's
  rotation includes the turn that makes it fit.
- **The proof always prints a fitting pose** if the search found one. `to_print` adds the
  best fitting pose when none of the finalists fits. `trial::overflows` uses the same
  exact turn.
- **`convex_hull` moves down into `core-geometry`.** `core-supports` and the fitting turn
  both use it (rule 5).
- **In the window, a turned model keeps the middle of its footprint.** It is slid only as
  far as keeps it on the plate, then dropped onto the plate and lifted.

## Consequences

The finalists of a model near the volume's size fit whenever any pose does. A part that
fits only at a slant is placed at that slant. On a million-face model the envelope costs a
pass of 256 dot products a vertex, and the check about a millisecond a candidate.

The envelope's margin can call a pose that fits by under 1 % of the radius too big. The
2 mm base margin is a guess for profiles without a raft. A proof that finds a finalist
the search said fits overflowing is the signal to revisit both.

## Alternatives considered

### The minimum-area rectangle alone

The standard use of rotating calipers puts one side of the box along a hull edge. A long,
thin part fits a square plate only near its diagonal, with no side along an edge, so this
would miss it.

### Sampling turns every few degrees

This is simpler, but a tight fit open for less than a degree is missed. The exact solve is
about forty lines.

### The full convex hull of the mesh

It is exact, but on a million faces it costs as much as the whole first pass, and many of
its vertices add nothing to any extent the check reads.

### The envelope, and what it costs

It reads extents a hair short, which the margin pays for. A pose that fits by less than
that is called too big.
