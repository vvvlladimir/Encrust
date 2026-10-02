# 0120. A printer's resins are its own presets, edited in place

- **Status:** Accepted
- **Date:** 2026-09-28

## Context

A resin file carries its own numbers and a table per machine (ADR 0050). The window edited
printers and resins on two pages with Save, Revert and a switch deciding whether an edit
went to the machine or to the resin; a machine with no table was offered numbers nobody
measured on it. Users think of resins the way every other slicer shows them: presets of a printer.
The formats also carry rests at the three points of a peel, and a price turns weight into
cost.

## Decision

One Printers page: the printers, and under the one open, the resins set up on it — the
resins with a table for it. Picking either opens its form, and every edit is written as it
settles; there is no Save. A printer's resin can be renamed, duplicated or taken off it.
Taken off, it stays in the pool: every resin file, from which any printer can add it back,
starting from the table of `last_printer`. A copy, a new resin and a renamed resin that
other printers share become a file of this printer alone, so one printer's presets never
rename another's. The plate's resin menu lists the printer's own; a printer switched to
brings one of its own. Ids come from the name when a profile is made and never change.
`Waits` — a light-off delay or three rests — are per machine, written into `.goo` and
`.ctb` and counted into the time; `[details]` — type, colour, price — belong to the resin.

## Consequences

The resin in a printer's list is exactly what that printer prints with. An empty table now
means "set up here". A shipped profile edited in place becomes the user's copy under the
same id and cannot be thrown away, since it would only come back; only the user's own
printers, resins and support profiles can be deleted.

## Alternatives considered

### A resin file per printer

One file per resin and printer is closer to how other slicers store them, but the shipped
resins would be
copied five times, and the pool — one resin for many printers — would be a search rather
than the catalogue.

### Keep Save and Revert

Explicit, but two steps for every number, and a form that could leave unsaved edits behind
when the list moves on.

### The option that won, and what it costs

A value is written while it is still being typed, so a mistyped number is on disk until it
is corrected; there is no undo on this screen.
