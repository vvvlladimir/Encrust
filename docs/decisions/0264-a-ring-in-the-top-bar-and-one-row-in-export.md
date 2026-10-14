# 0264. Put a ring for the current printer in the top bar, and leave Export one row

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

Since 0263 a printer's whole panel lives on a card in the Export tool's 328 point
inspector. Those cards are too cramped for one printer and unworkable for several. They are
also out of sight unless Export is the open tool, which is the opposite of what a print
running for hours needs.

## Decision

- Left of the print chip, once the window knows any printer, a ring as tall as the chip
  stands for the current printer. That is the one the printer in hand is bound to, else the
  first one printing, else the first one listed (`Network::current`). The ring's arc is the
  share done, in the colour of what the printer is doing: accent printing, warning paused,
  danger silent. While a print runs without a count it spins. The share is written inside;
  otherwise the printer glyph is. With more than one printer a small badge gives the count.
- A press opens the printer's sheet when there is one printer. With several, it opens a
  list first: a row per printer with its small ring, name, state and time left, and a row
  opens its sheet.
- The sheet is a modal over the window, 620 points wide. On the left are the picture of the
  plate and the camera; on the right the state, the print with its bar, Pause or Resume
  and Stop, the readings and the film. Below both are the prints it remembers, read when the
  sheet first draws them, as tiles several to a row that scroll when the window is short.
- The Export tool keeps one row for where files go: the bound printer's name and what it
  is doing, opening a list of every printer that speaks the profile's protocol to bind
  another, and beside it a button into the bound printer's sheet. The cards of 0263 are
  gone.
- The ring keeps the network fresh on its own, so a printer appears without Export being
  opened. The browser build draws no ring, because it reaches no printer (ADR 0182).

## Consequences

What a printer is doing is visible from every tool, and one press away in full. There is
one place for a printer's detail, the sheet, and the Export tool is back to sending.

The window now scans every 30 s for as long as it is open, not only while a list is in
view: a two second broadcast each time, plus a watch thread per printer found.

## Alternatives considered

### A rail tool for printers

It would sit beside the plate's tools, and opening it would close the tool in hand. A
print is not part of the plate.

### A window of its own with a list and a detail pane

The fullest view for many printers. It costs a second OS window, and with one printer it
is a list of one in the way of the detail.

### The option that won, and what it costs

A modal hides the plate while it is open, and the top bar gives up 38 points the chip had.
