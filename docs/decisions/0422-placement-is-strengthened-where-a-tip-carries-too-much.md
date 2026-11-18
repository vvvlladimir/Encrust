# 0422. Strengthen automatic placement where a tip carries too much, on tips no thinner than 0.30 mm

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

Placement spaces supports by geometry: a spacing, an overhang angle, and the size an automatic
profile picks from what a contact holds (ADR 0325). Nothing checks that a tip survives the
film's pull on the layers it carries. A printed coupon calibrated the limit a tip tears at,
`NECK_LIMIT_MPA` 13 MPa (ADR 0417), and the bench reads each tip's stress against it
(`docs/design/analysis.md`, "What each tip carries"). On thirteen models a few tips read under
×1.5 — the first tips of thin parts and tiny islands — and a uniform heavier preset did not
mend them: it costs resin and scars everywhere.

Prints at 0.05 mm showed a 0.24 mm tip does not print and a 0.18 mm one less; Auto shipped
both, for its lightest and its steadying shapes (ADR 0325, 0326).

## Decision

- **Each tip's load** is read by `core_analysis::TipLoads`: the model's stack in print order,
  the tips under each part as one section of discs, each layer's pull shared out with its peel
  lever on the ellipse of the piece's own second moments. `core_engine::carried` cures the
  stack on an unmirrored panel of 0.05 mm pixels laid over the plate, so no printer is needed.
- **`core_engine::strengthen`** runs after every automatic placement — Generate in the window,
  `--supports` and recipes on the command line through `stand_under`: a tip under ×1.5 is
  built one size heavier (`Need::heavier`) where its plan has a wider tip after its own; at
  the widest, under one profile, or for a steadying support, it gets a neighbour half a
  spacing away on an underside facing down, seated no higher than its worst layer and off
  anything blocked. At most three rounds; a round that changes nothing ends it. Supports
  already standing are read as tips of the placing profile and never changed.
- **Auto's tips** are 0.30, 0.35 and 0.40 mm, small pillars alike; its steadying shape is thinner (ADR 0424).
  Light stays at 0.30.

## Consequences

Over the bench's thirteen models no tip of any preset keeps under ×1.5; it was five, all on thin
parts. Placement costs one more cure of the model a round: 0.2 s more on a figure of 1.5
million faces. Contacts and resin are within 1 % of before. A neighbour can land on a thin
part, where a print may scar it; Light and Auto at 0.30 mm still break in the wash at times.

Reopen with a second resin or layer height, when a tip the read called safe tears, or when the
neighbour's place on thin parts matters (supports along a thin part's length are not placed).

## Alternatives considered

### Place by force from the start

Each layer's pull shared out as placement climbs, the spacing chosen so every tip carries the
same. More exact, but a rewrite of sampling and placement for five weak tips of 52 runs.

### A switch in the window and a flag

Strengthening is cheap and only ever adds; a switch is a line of interface, its strings and
pictures, for a choice nobody would turn off.

### The printer's own panel

Reading on the printer's pixels matches the coupon exactly, but every placement call would
need the printer, and a run without one would place differently. At 0.05 mm a 0.30 mm tip is
six pixels; the load moves by under 2 % on the bench's closed-form slabs.

### The option that won, and what it costs

A tip is mended where it stands, not placed right in the first place: a cluster of weak tips
grows by neighbours one round at a time, and three rounds may leave one weak. A neighbour is
looked for in four directions only, so a narrow part may have no room for one.
