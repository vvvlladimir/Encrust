# 0196. A resin is measured, not shipped, and a copy says where it came from

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

The catalogue shipped four resins. None was measured on anything: the files carry an
`UNVERIFIED` header and a table per machine transcribed from vendor material, and ADR 0050
already says an exposure is only true on the machine it was measured on. A manual run
found what that costs:

- a fresh install reaches `Add resin → From the pool` and is offered four resins it can
  neither delete nor trust, and a resin it made and never edited stays in that pool;
- the CLI fell back to `default_resin_for`, so `slice --printer <id>` with no `--resin`
  wrote a file at numbers nobody measured, and said nothing;
- `standard-grey` in the user's own directory, with no table for a machine the shipped
  file does cover, warned "carries no numbers" — the shipped file was there, under the
  copy, unreachable.

The last one is not about resins. ADR 0158 made an installed profile a copy of the shipped
one and named the trade: a correction in a later release does not reach a user who
installed the old one, "revisit if shipped numbers start moving often". They have: a copy
of the Saturn 4 Ultra taken before ADR 0142 still claims `per_layer_settings`, which stalls
the plate on a tilting vat, and nothing in the window says the copy is old.

Deleting a profile is the other half. Everything on the Settings screen is written as it
settles and there is no undo on it (ADR 0120), and *Remove this machine*, *Take off this
printer* and the resin pool deleted on one click.

## Decision

**The catalogue ships no resin.** `assets/profiles/resins/` is gone, the build script
treats a missing directory as an empty table, and a resin is only ever the user's own.

- A printer with no resin on it says so and offers **Add the first resin**, which makes one
  to type a measured exposure into. There is no neutral starting point to pick instead.
- The window refuses to slice without a resin, as it refuses without a printer. The resin
  picker reads *Select a resin* rather than naming the stock numbers.
- `encrust slice`/`estimate` with `--printer <id>` and no resin is an error naming
  `--resin` and `--material`. A run that names no catalogue machine keeps the stock numbers
  it has always had, and a report over geometry — `inspect` — still runs without a resin.
- A resin can be deleted from the pool, and one taken off its last printer goes with it: a
  resin on no printer is in nobody's pool.

**A deletion on the Settings screen is asked about first**, naming what goes and what goes
with it — the printers a shared resin comes off, and whether removing a machine leaves it
in the library.

**An installed copy says what it is.** The printer form states that the machine is the
user's copy of one this build also ships and that the two differ, and offers *Use the
shipped profile*, which writes the shipped numbers back over the copy.

## Consequences

Nothing in the window or in a file is an exposure nobody measured. The first run costs one
more step — a machine out of the library, then a resin with numbers off an exposure test —
and that step is where the numbers that ruin a print are decided anyway.

Every example in `docs/cli.md`, `README.md` and `fuzz/README.md` now names a resin of the
reader's own, so no documented invocation runs as typed on a fresh install. That is the
honest state of it; a worked example cannot carry a measured exposure.

The stale-copy note is a statement, not a merge: it fires for a profile the user edited on
purpose as much as for one a release left behind, and *Use the shipped profile* throws the
edit away. Revisit if users start keeping edits they lose this way — the fix then is a
field-by-field comparison, which is a page, not a line.

## Alternatives considered

### One neutral resin, shipped

A single `generic-resin` so that a first run can cut something. It lost on the same ground
as the four: the numbers are invented, the window would open on them, and a user who never
runs an exposure test never finds out. It also keeps the pool's undeletable entry.

### Keeping the shipped resins and only adding a delete

The smallest change: delete from the pool, and leave the four in the library. Rejected
because the pool is where a shipped resin gets added back the moment it is thrown away, and
because `default_resin_for` would still hand the CLI an invented exposure.

### A profile that updates itself from the library

Instead of a note, re-read the shipped file and merge what the user never touched. It needs
to know which fields the user touched, which means keeping the copy and the original side
by side — a format change for a case that a one-line note and a button cover.

### The option that won, and what it costs

A first run cannot print anything until the user has run an exposure test, which is a wall
in front of someone who only wants to see a file come out. The window says what to do at
every point it refuses, and that is the whole of the mitigation. The confirmation dialog is
also the first modal question the application asks; one more and it needs to become a
widget rather than a screen of its own.
