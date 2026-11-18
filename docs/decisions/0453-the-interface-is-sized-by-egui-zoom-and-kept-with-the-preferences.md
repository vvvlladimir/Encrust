# 0453. Size the interface by egui's zoom, kept with the preferences and capped by the screen

- **Status:** Proposed
- **Date:** 2026-11-16

## Context

The window is drawn at the scale the system gives each display. A user on a large monitor far
away, or with weaker sight, wants it larger; one on a small laptop screen, smaller. egui already
multiplies the system's scale by a zoom factor and answers Cmd +, Cmd - and Cmd 0 itself, but
keeps the factor for the run only, and those keys sit outside the shortcut table that ADR 0106
makes the one list of every key.

The window's smallest size, 900 × 560 points, is what the panels are laid out for. At a larger
zoom the same pixels hold fewer points, so a window left at its smallest would lay them out
narrower than designed, and a large zoom on a small screen would ask for a window the screen
cannot hold.

## Decision

`General::interface_scale_percent` is a preference, one of 75, 90, 100, 110, 125, 150, 175 and
200 %, chosen on Settings → Appearance and stepped by `Action::Scale` in the table: Cmd = and
Cmd + larger, Cmd - smaller, Cmd 0 the system's size. Those keys act over any window that holds
the keyboard, a focused text box too, which keeps every other key (ADR 0229). egui's own
keyboard zoom is turned off.

Each frame `encrust_app::interface_scale::apply` hands egui the scale as `zoom_factor`, and on
a change sends the window's smallest size in the new scale, so the system grows the window
rather than squeezing the panels. The scale drawn is held where the smallest size still fits
the monitor, never below the system's own for that reason; the preference keeps what was
asked, and the page names the limit. The 3D view keeps its resolution, following the pixels;
its gizmos and labels, sized in points, grow with the rest.

## Consequences

Every widget, font and token grows together; nothing in `ui::theme` has a second set of
sizes. A larger interface leaves less room for the plate at a given window size. In a
browser the keys size the window, not the page; the browser's own zoom still works from its
menu, and the two multiply.

Reopen if a user needs a scale between the steps, or a screen's size proves too coarse a
limit once the taskbar or the menu bar is counted.

## Alternatives considered

### Scale the theme's sizes instead

Larger fonts and spacing in `ui::theme` keep the pixels sharp but leave every hand-placed
size, glyph and picture behind, and double the tokens to maintain.

### Leave egui's keys as they are

Nothing to write, but the size is lost at every start, cannot be found in Settings, and two
keys answer outside the table and the sheet.

### Zoom by egui, and what it costs

A zoom between whole pixel ratios draws hairlines and glyph edges a little softer than the
system's own scale does.
