# 0221. Float the side columns over a stage the width of the window

- **Status:** Accepted
- **Date:** 2026-10-11

## Context

ADR 0217 docked four columns down the window's sides — plates 44, plate panel 272,
inspector 328, rail 68 — each the full height under the top bar, and docked the layer strip
under the stage. On a laptop that left the viewport about a third of the screen, while most
of every column was empty: the Select inspector holds four rows, a plate of one model holds
one. ADR 0102 had made the plate a panel because a card over the viewport needed last
frame's rectangles to keep a press on it from orbiting the camera, because the plate tabs
vanished with the card, and because the card capped its list at 260 points. ADR 0193 since
made whichever egui layer is under the pointer own it, and a folded card can keep its
plates, so the first two reasons are gone.

## Decision

The stage is everything under the top bar, and the viewport draws all of it. Everything
else is an `egui::Area` card over it, laid out by `panels::Window::columns`, `FLOAT_GAP`
(8 points) from the stage's edges and from each other, every card's top on one line:

- **Left:** the rail at the edge, then the inspector. A tool is picked and its panel opens
  beside it. The rail is always the stage's height: on a tall screen its gaps spread up
  to a ceiling and the rest stays empty; on a short one the tools' names go first, then
  the gaps close to their floor, and only then does it scroll (`tool_rail::Fit`).
- **Right:** the layer strip at the edge, the stage's full height and 56 points wide, its
  top the top of the print: the three views, the layer and its height, a layer up, the
  track, a layer down, and play. The track carries the cured area as a profile either side
  of the rule and a tick at each risk the check found, in the model view too while the
  stack is still the plate's. Beside it two cards as wide as each other: the models — the
  row of plates at its head, the list, their actions — from the top, and this plate's
  figures over Slice standing on the stage's foot.
- **Between:** the view cube at the top right, the view tools at the bottom right beside
  this plate's card, the notices top centre, the empty plate's card in the middle. Preview splits this room for the mask,
  and the mask's pane runs on under the plate cards.
- **Height.** A card is as tall as what it holds. Past the stage's height the part that is
  a list scrolls — the tool's sections, the plate's models, the plates — while a tool's
  action and the models' actions stay in view under it (`ui::body_and_foot`). The models
  card stops above this plate's.
- **Edges.** A drag on the inspector's or either plate card's inner edge still sizes it,
  and past its floor still folds the models card down to its plates and a button that
  brings it back, this plate's card with it; the handle is an area of its own just outside the card, so the camera under it
  does not orbit.
- The sheet of keys and the gear move from the rail's foot to the top bar's right end.

This supersedes ADR 0217's docked columns, its sides — the rail and inspector move to the
left, the plate to the right — its plate strip, its horizontal layer strip under the stage
and the two buttons at the rail's foot, and ADR 0102's plate panel; 0102's fold, its drag
handles and its rail beside the inspector stand.

## Consequences

- The viewport is the window less the top bar. What a card does not hold is model.
- A vertical track is as long as the stage is tall, longer than the room a horizontal one
  had between the columns, so a layer is a finer drag.
- A card covers the model under it; the orbit, the fold and a narrower column are the
  answers, as a docked column's were.
- A card is drawn to last frame's measure of its foot, and a frame whose foot changed
  height is drawn twice.
- The plate is framed to the middle of the window, not of the room between the columns.
  Reopen with an off-centre projection if a framed model hides behind a card.

## Alternatives considered

### Keep the columns docked and fold them

The inspector already has a greyed fold. Folding gives the room back only while the
column is shut, and every tool needs the inspector open.

### Tools along the top, plates and layers along the foot

Tried first. Two rows of cards over the stage left the side cards a stage less two rows
tall, and put a picked tool a screen's width from its panel.

### Float the columns at full height

A floating card that is always as tall as the stage changes nothing but the corners.

### The option that won, and what it costs

The cards change height as tools and selections change, so the window moves more than a
docked one did, and a card standing on the model hides part of it where a column only
narrowed the view.
