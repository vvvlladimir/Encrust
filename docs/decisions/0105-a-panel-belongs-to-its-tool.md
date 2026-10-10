# 0105. A panel belongs to its tool

- **Status:** Superseded by 0118 (the transform switch only) and 0217 (the footer only)
- **Date:** 2026-09-27

## Context

The inspector drew the open tool's panel and then appended Slicing and Estimate to
whatever it was. Painting supports showed layer height, adaptive layers, anti-aliasing and
the output format under the brush. Transform carried a Copies block that had nothing to do
with the three numbers above it, and the rail spent three of its entries on Move, Rotate
and Scale — one idea, three buttons, none of which means anything until a model is picked.

## Decision

The right-hand column shows exactly one panel: the open tool's. Everything that was
appended to it moves to whatever owns it.

- Layer height, adaptive layers, anti-aliasing and the exposure bands become a tool of the
  rail, so they are opened rather than always present.
- What the stack comes to shrinks to two figures in the inspector's footer, over the
  button they inform.
- The container the file is written into moves onto that button: `Slice to .goo` with a
  caret beside it, because the format is a property of that one action.
- Duplicate, mirror, array, arrange and remove act on the plate's contents rather than on a
  tool, so they are a row of icons at the foot of the plate panel.
- Move, Rotate and Scale leave the rail and become a segmented switch inside the Select
  panel. `Tool::is_placing` keeps the rail's Select button lit while one of them is out.
- The machine, the resin and the format are written to `preferences.json` beside the user's
  profile directory the frame they change, so the next run opens on them.

## Consequences

- A panel can be read without scrolling, and no tool answers a question the user did not
  ask.
- The rail is one entry shorter and each entry is a thing to do, not a mode of one thing.
- Layer height is now two clicks away while any other tool is open. That is the trade:
  it is a decision made once a print, not a knob turned while supporting a model.
- The format is only visible on the Slice button. A user looking for it in a panel will
  not find one.
- Only catalogue ids are remembered. A printer opened from a file has to be opened again.

## Alternatives considered

### Keep the shared blocks, collapse them

An `egui::CollapsingHeader` over Slicing and Estimate would have hidden them without
moving them. Rejected: a collapsed block is still the wrong column's, and the user pays
for it with a click every time the inspector is redrawn from a fresh state.

### A fourth mode for slicing

Prepare, Preview and a Slice mode would have given layer height a screen of its own.
Rejected: a mode is a way of looking at the plate, and this is a panel of numbers.

### The option that won, and what it costs

One panel per tool is a rule that holds only as long as nobody needs two things at once.
It puts the layer height behind a tool button, and it spreads what used to be one column
over three places — the rail, the plate's footer and the Slice button — so a user who has
learned the old inspector has to learn where each piece went.
