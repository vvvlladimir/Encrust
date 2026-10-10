# 0217. The window is a top bar, a flat rail, a docked inspector and a layer strip

- **Status:** Accepted; the return to the model on a tool press is superseded by 0218, the
  docked columns and layer strip by 0221
- **Date:** 2026-10-10

## Context

The window had grown five bands of chrome: a 28 or 36 point title strip with the menus and
a centred Prepare/Preview switch, a 30 point strip of plate tabs under it, a 26 point
status strip at the foot, a vertical section rail floating over the viewport, and an
inspector footer holding Slice, Send, Start print and every progress bar. Preview hid the
rail and the plate column, so the window jumped on every switch. The v2 design gathers the
same controls into fewer places and keeps every column in view.

## Decision

The window is, in `panels::Window::show` order: a 44 point top bar; the 68 point rail at
the far right, a glyph over each tool's name; the inspector beside it, holding only the
open tool; a 44 point strip of plates at the far left, the plate column beside it; and
the stage, with a 52 point layer strip along
its foot. All three columns are drawn in every view.

- **Top bar.** The File, Edit and View menus at its left end. At the right end: a chip
  naming the machine and the resin, each opening its list, and the layer height and
  exposure, which open the Layers tool; Find, drawn and not yet working; and the window
  buttons. The bar is 44 points on macOS too, and the system's own buttons are moved onto
  its centre line through `objc2-app-kit` (`traffic_lights.rs`), checked every frame
  because AppKit lays them out again on a resize and on leaving full screen.
- **Plate strip.** One numbered square per plate, its name and model count on hover, a +
  under them; a right click duplicates, removes, or moves the selection onto it. Plates
  are not renamed in the window: a number is all the strip has room for.
- **No status strip.** A running job — slicing, sending, cutting the preview — is a card
  top centre of the stage with its progress and a Cancel. Under it come a sent file
  waiting for Start print, then the last message: a failure stays until put away, anything
  else stands for four seconds. The tally of the plate moves to the plate column.
- **Plate column.** Models with their count and a +, the list, the plate's actions, then a
  summary — the plate's figures, or the layer's own in Preview — over Slice. A caret beside
  Slice holds Slice all plates and Slice and send, with whether the bound machine answered.
- **Layer strip.** Three views: Model, which is the Prepare mode; Side by side and Layer
  mask, which are Preview with the mask taking half the stage or all of it. Then the
  transport, a horizontal track carrying the exposure bands, the cured-area profile and a
  tick per risk, and the layer and height it stands at; a click on the layer types one to
  go to. `workspace::Mode` stays; only what
  switches it moved.
- **Rail.** Pressing a tool from Preview returns to the model, as the tool's key already
  did. A tool is lit only in the model view. The sheet of keys and the gear stand at the
  rail's foot, unlabelled.

This supersedes ADR 0025's status strip, 0061's floating rail, 0102's plate strip and the
pickers at the foot of the plate panel, 0103's hiding of the columns, 0104's height of the
strip and its 68 points left to the lights, 0105's inspector footer, and 0157's Send button beside Slice.

## Consequences

- The strip of plate tabs and the status strip give the stage 56 points, and the layer strip takes
  52 of them back for a control that was floating over the model.
- Every column stays put across a change of view; the stage alone is re-divided.
- A message no longer has a permanent home. An error waits for the user; the trail of
  informational messages is gone, which is the price of a quieter window.
- Slice and send is one press deeper, behind the caret. Reopen if sending becomes the usual
  destination.

## Alternatives considered

### Draw our own window buttons on macOS, or a 28 point bar there

Our own buttons need no dependency but lose what AppKit's carry: the glyphs on hover, the
green button's tiling menu, a long press into full screen. A bar the native height is
shorter than the chip and the menus need.

### Keep the status strip and add the bar

The least change, but it keeps a band of chrome whose left half is usually "Ready" and
whose right half belongs with the plate.

### The option that won, and what it costs

The plate column now shows controls
that edit the plate while a stack is read, so a change there makes the preview cut again,
and the buttons on macOS are moved behind AppKit's back: three Objective-C crates become
direct dependencies, already in the tree through eframe, with two `unsafe` calls a release
of macOS that renames the title bar's views could break.
