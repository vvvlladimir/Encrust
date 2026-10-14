# 0268. Undo reaches the profile form in view

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

The printer, resin and support-profile forms write every edit as it settles, with no Save
(ADR 0120), and that ADR's cost was that a mistyped number had no undo. Both windows are
modal and swallowed Ctrl+Z and Ctrl+Shift+Z, so the keys that work everywhere else did
nothing in the place where a number is most often mistyped. The plate's history cannot
take these edits: it snapshots the scene and the tools (ADR 0086, 0192), and a profile is
neither — it is a file in the catalogue, shared by every plate.

## Decision

Each open profile form, a `settings::Draft`, keeps its own history: the values before each
settled edit, up to 64, and what undo took away. An edit is recorded the way the plate's
are, once the pointer is up and no control holds the focus, so a drag or a typed number is
one entry. While the Machine and resin window or the Settings window is open, undo and
redo go to the form in view — the resin, the machine or network page of a printer, or a
support profile — and nothing else; the form then writes what it holds as it writes any
edit. The history lives as long as the draft: opening another profile starts a new one.
Deleting, renaming, duplicating and taking a resin off a printer stay outside it and keep
their question (ADR 0196).

## Consequences

- A mistyped number is one keystroke from being put back, on disk as well.
- A form's history is lost when another profile is picked or the window is closed.
- A resin's carried exposure (ADR 0128) is forgotten on undo, since the values it would
  put back are no longer the ones in the form.

## Alternatives considered

### Put profile edits on the plate's undo stack

One stack for the whole window, but undoing a stroke on the plate would rewrite a profile
file that other plates print with, and a profile edit would sit between plate edits.

### The option that won, and what it costs

Two histories in one window, and the keys answer whichever is in view; a deletion is still
final.
