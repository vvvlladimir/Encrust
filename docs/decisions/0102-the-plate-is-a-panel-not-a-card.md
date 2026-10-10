# 0102. The plate is a panel, not a card over the viewport

- **Status:** Accepted, the plate strip and the pickers superseded by 0217, the panel by 0221
- **Date:** 2026-09-26

## Context

ADR 0025 put the plate's contents in an `egui::Area` floating over the viewport, the way
other resin slicers do. That ADR recorded the cost in its own Consequences: the viewport
reads the raw pointer before the cards are drawn, so `panels::Overlays` has to keep last
frame's rectangles and refuse an orbit that began on a card. A frame of lag, and a hack.

Two things have changed since. Step 14b put several plates in a project, and their tabs
went inside the card, so a project's plates disappear whenever the card does. Step 15a made
the selection an ordered set, and a multi-model selection has nowhere to report itself. The
list itself was already capped at 260 points and scrolling under its own header.

The FDM slicers moved their scene browsers out of the viewport into a collapsible panel of
their own for the same reasons.

## Decision

The plate's contents are `egui::Panel::left`, 228 points wide by default and dragged wider
or narrower by its own edge. Dragged past its floor the panel folds away and leaves a
hairline down the stage's edge that lights up under the pointer and brings it back on a
click. The inspector is dragged the same way, though it never folds: it carries the one
action the window is for.

Neither column uses egui's own resizable panel. A resizable panel grows to whatever its
contents ask for, up to its ceiling, and then remembers that width — so opening the
Supports tool would widen the inspector and leave it widened. Both are `exact_size` at a
width this crate owns, with one drag handle painted over the boundary afterwards.

The panel holds the model list and, at its foot, the printer and the resin: those belong to
the plate rather than to whichever tool is open, and moving them there leaves the inspector
to the tool alone. What an import had to repair, and what it could not, is a popup on the
model's own row rather than a block unrolled under it, because a plate of six models would
otherwise be six reports deep.

The plate tabs move to a 30 point strip of their own under the title, with the tally of what
is on the plate and how much of it is selected at the right end of that strip.

The tool rail moves from the left of the window to the right of the inspector. A tool and
the panel it owns are one control, so they stand side by side, and the left edge is then
the plate's from top to bottom.

This supersedes the scene-card half of ADR 0025. The rest of it — fixed panels, no dock,
the section rail, the view tools — stands.

## Consequences

- `Overlays` keeps three rectangles instead of four, and none of them is the list. A press
  on the plate can no longer orbit the camera at all, rather than not orbiting it a frame
  late.
- The list is as long as the plate is full: it scrolls in the room the panel has rather
  than in a 260 point window inside a card.
- Folded, the panel gives the stage more room than the card ever left it, because a card
  covers the model it floats over and a panel does not.
- The window is 228 points narrower for the stage while the panel is open. On a small
  display that is real, and the fold is the answer to it.
- Both columns are now a width the user owns, which egui remembers per panel id. Nothing
  else in the window is adjustable, so this is the one place a layout has any state.
- `panels/scene_card.rs` became `panels/scene_panel.rs`, the tabs became
  `panels/plate_bar.rs`, and `panels/inspector/printer.rs` is gone into the plate panel.

## Alternatives considered

### Keep the card and widen it

Cheapest. Rejected because it fixes none of the three complaints: the tabs still vanish
with the card, the list still cannot grow, and `Overlays` still has to protect the pointer.

### Keep the card but let it dock to the edge

A card the user can pin. Rejected as a setting standing in for a decision: it doubles the
layout code and the pointer rules, and every user would have to find the pin before the
window worked properly.

### Leave the tool rail on the left

No change to muscle memory, and it matches the FDM slicers. Rejected because with the
plate on the left as well, the left edge would carry two unrelated columns and the tool
would sit as far as it can be from the panel it drives.

### The option that won, and what it costs

A panel is a rectangle the stage does not get. The card was free in that one sense, and a
user who wants the largest possible viewport now has to fold the panel rather than getting
it by default. The rail's move also breaks the habit of every other slicer, and there is no
setting to put it back: that is the price of one unbroken column of chrome down the right.
