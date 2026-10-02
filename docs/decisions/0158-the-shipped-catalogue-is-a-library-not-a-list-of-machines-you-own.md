# 0158. The shipped catalogue is a library, not the machines you own

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

The window listed every shipped profile as though it were set up: five machines and four
resins, none of which the user has. It reads as a machine already chosen and a resin
already measured, and the Print block opens on numbers nobody tested on anything.

`0157` and the step before it put the catalogue on course for about a hundred and fifty
machines. A list of a hundred and fifty machines a user does not own is not a list.

## Decision

A profile is **installed** when it is the user's own copy — `Source::User` — and the
shipped profiles are the library it is installed from. A first run therefore has no
printer and no resin.

- The Machines list, the printer picker and the resin picker show installed profiles only.
- The `+` opens the library over every machine there is. Picking one writes a copy of the
  shipped profile into the user's directory under the same id, which is what makes it
  theirs to edit and to remove. **Custom printer** is the same thing with nothing in it.
- **Add resin** offers every resin the catalogue has that this printer is not set up with,
  the shipped ones included; adding one writes a user copy tuned for that printer.
- Removing a machine deletes that copy. A shipped id falls back to the library entry, so
  the machine is still there to install again; an id nothing was shipped under is gone.
- The catalogue itself is unchanged, so `slice --printer elegoo-mars-4-ultra` still
  resolves a shipped id without anything being installed.

## Consequences

The window states what the user has rather than what the build ships, and a machine in the
list is one somebody chose. Editing is simpler: everything listed is writable, so there is
no shipped-profile special case behind every field and every delete button.

The cost is a first run that cannot slice until a machine is taken out of the library. The
Settings screen opens straight onto the library when nothing is installed, and the pickers
say where to go, which is the whole of the mitigation.

An installed copy is a copy: a corrected shipped profile in a later release does not reach
a user who installed the old one. That is the same trade the user directory always made,
and it is what *Remove* then *Add* fixes. Revisit if shipped numbers start moving often.

## Alternatives considered

### A separate `installed` list in the preferences file

Ids in `preferences.json`, with the catalogue untouched. It lost because two places would
then say what the user has — the file and the profile directory — and they would disagree
the first time somebody copied a `.toml` in by hand.

### Shipping the catalogue as it was, and only hiding it behind a search

No new concept, and the search already narrows a long list. It lost on the resins: a
machine would still arrive with four resins measured on nothing, which is the part that
wastes a tank.

### The option that won, and what it costs

A first run is empty, and empty states are the ones nobody tests. Every list the window
has must now say what to do when it holds nothing, and a user who skips the library finds
a window that will not slice.
