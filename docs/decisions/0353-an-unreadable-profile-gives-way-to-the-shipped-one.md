# 0353. Leave an unreadable profile out, use the shipped one in its place, and offer a reset

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

ADR 0049 lays the user's profile directory over the shipped catalogue, and a file there that
does not parse is reported. Reporting means the catalogue fails to load, so one bad file
leaves the user without a single profile. Releases tighten validation and retune the shipped
presets, and the window keeps a full copy of a preset in the directory even if nobody changed
it. So a release can make a copy the user never touched fail validation, and the start page
opens on an error and nothing to slice with.

## Decision

**A file in the user's directory that does not read is left out, never fatal.** Whatever the
catalogue already held under its id, the shipped profile if one ships, stays in use. The
catalogue answers each file it left out as an `Unreadable`: kind, id, path and the error.
This covers every kind, the tool presets included, which already worked this way.

**A reset throws away every unreadable copy of a shipped profile.** A file under an id nothing
ships is the user's own and stays on disk, since there is nothing to put in its place.
Writing or forgetting a profile takes its id off the list. The window shows how many files
were left out, and why behind the question mark, on the start page and over the stage. It
offers the reset and asks before doing it. `encrust profiles reset-unreadable` runs the same
call.

## Consequences

A release that makes old copies invalid no longer locks anyone out. Someone who edited a
preset that now fails loses those edits to the shipped preset until they fix the file or
reset it. Their file is kept, and the notice names it. A broken file of the user's own keeps
the notice up until they fix or delete the file by hand.

Reopen this when profiles are kept as differences from the shipped preset rather than whole
copies. Then a newer release would seldom make a file unreadable at all.

## Alternatives considered

### Keep failing the catalogue, and say which file

The message would be accurate and the window would still have nothing to slice with. The
person who has to act is the one least able to find the configuration directory.

### Reset unreadable copies silently on load

No question asked, but a hand-edited file disappears without the user knowing. Undo cannot
bring back a deleted file (ADR 0268), so the window asks first.

### The option that won, and what it costs

Two outcomes are possible for a profile id: the user's file, or the shipped profile standing
in for a file that is still on disk. The notice is all that tells them apart, and a list of
left-out files becomes part of the catalogue's state, kept current by every save and forget.
