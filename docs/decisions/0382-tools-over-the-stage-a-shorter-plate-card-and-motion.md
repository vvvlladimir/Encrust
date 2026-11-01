# 0382. Stand the tools over the stage, shorten the plate card, ask by a second press, and move

- **Status:** Proposed
- **Date:** 2026-10-29

## Context

The window had drifted screen by screen: the tool rail over the inspector pushed every
panel down by its height, the plate card stated seven figures in dotted rows over its one
press, some panels printed how to work the viewport as paragraphs, a bin asked twice but a
deleted profile or machine did not, and nothing moved, so a fold or a window opening
jumped. The standard said how a panel behaves and the design system what widgets exist,
but no page said where each part of each screen stands and how big it is, so each panel was
judged alone.

## Decision

- **One written layout.** `docs/design/ui-screens.md` states every screen's parts, their
  place and size; the design system gains the state matrix and the glyph table. A change is
  held to the three pages; where the window differs it is a known gap of `ui-standards.md`.
  The layout was worked out on an HTML sketch kept out of the repository; the pages, not the
  sketch, are the record.
- **Tools over the stage.** The rail is one row standing over the top of the stage between
  the two side cards, as wide as the room between them, 8 points from each; it no longer
  sets the inspector's width, which starts at the stage's top. Each tool is a glyph over
  its name, the digit of its key shown while the pointer is on the row; a window too
  narrow drops the names. This supersedes ADR 0348's rail over the inspector.
- **The plate card** states time, resin, weight and price in a two-by-two grid, each name
  over its figure. The split between model and supports, vertices and triangles open under
  them from a *Details* fold remembered in `ui::kept`; height and layers are the layer
  strip's and are not repeated. In Preview the same grid states the layer's exposure, its
  block, cured area and volume. The card stands on the stage's foot, so it grows upward.
  Slice and send, or Start print, is the primary; Slice to a file is a square icon button
  beside it, as tall as it. This supersedes ADR 0348's Slice to a file under the primary.
- **An icon beside a bigger control** is a square as tall as that control, centred on it.
- **The irreversible asks by a second press**, not a dialog: a profile, a machine, a resin
  taken off a machine, a saved preset, a group with its supports, every support, Stop on a
  running print. The first press turns the control `danger` with a question mark for
  three seconds; Esc, another click or the time puts it back. What undo takes back acts at
  once.
- **Viewport instructions** are a hint bar over the stage under the tool row — key, then
  what it does — never a paragraph in a panel.
- **Motion** shows a change of state and nothing else, from `theme::motion` only, and is
  off when *Reduce motion* in Settings → General is on.

## Consequences

- A panel gains the rail's height; the stage loses a strip at its top to the tool row, over
  the plate's far edge where little is drawn.
- The inspector's width is its own again: a width dragged under 0348 is forgotten once.
- A new screen part or state is written into `ui-screens.md` before or with its code, so
  that page is one more file a visible change touches.
- Glyphs stay Phosphor regular from `egui-phosphor`, already compiled in: the glyph table
  names constants of `egui_phosphor::regular`, and nothing new is installed.
- Reopen if the tool row hides the plate on a short window, or users miss the figures
  moved under *Details*.

## Alternatives considered

### Keep the rail over the inspector and restyle it

Smallest change, and the tool sits over the card it opens. It still costs every panel the
rail's height and ties the inspector's width to the length of the tool names in a language.

### Keep an HTML mockup in the repository as the target

A picture is quicker to read than a table of sizes. It is a second drawing of the window in
another toolkit that drifts silently, needs a browser and a CDN, and cannot be checked by a
test; the panel pictures of ADR 0215 already show the window as built.

### A vertical rail down the stage's left edge

Every tool stays put and the inspector keeps its height, but it takes 68 points of width
from the stage and spreads nine tools down a tall screen, the cost 0348 left it for.

### The option that won, and what it costs

The tool row is a reach from the inspector to the stage's top centre, and it covers a strip
of the stage. A layout in words and numbers is slower to read than a picture, and a reader
new to the window has only the panel pictures to see it by.
