# 0197. A resin taken off a printer waits in the pool

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

ADR 0196 made a resin the user's own and nothing else's, and ruled that one taken off its
last printer goes with it: a resin on no printer was in nobody's pool, so keeping the file
left something nothing reached. A run on the Settings screen showed what that costs. An
exposure measured over several test prints is the most expensive thing in the catalogue,
and *Take off this printer* threw it away the moment the last machine let go of it — the
machine being removed, or renamed, or the user simply tidying which resin sits where. The
question before it said so, but a question is not a safeguard: there is no undo on that
screen (ADR 0120), and the file was gone.

The opposite error is as easy to reach. **New resin** writes the stock numbers under the
name *New resin* straight away, so a change of mind one click later leaves a file carrying
nothing anybody measured — exactly what 0196 cleared out of the catalogue.

## Decision

**A resin outlives the printers that had it, and the pool is where it is deleted.** Taking
one off a printer removes that printer's table and nothing more. On no printer it stands in
the pool, offered to every machine, and *Delete this resin* in the pool is the only thing
that throws it away.

The exception is a resin nobody ever typed into — the stock numbers, the stock name, and no
machine's table carrying a change (`settings::is_untouched`). Taking that off the printer it
was made on throws it away, so **New resin** and a change of mind leave no file behind.

Two things follow for the window:

- A printer with no resin on it offers the single **Add the first resin** button only when
  the pool is empty too. With anything in the pool it is the ordinary *Add resin* menu,
  because there is now something to pick.
- The question before a deletion names the verb it answers — *Take off*, *Remove*,
  *Delete* — and says "This cannot be undone." only when it is true. Taking a measured
  resin off a printer is undone by adding it back from the pool.

## Consequences

An exposure is lost only where the user asked for it in the words that say so, and a pool
is the one place to look for a resin not on a machine. The cost is a second step: a user
who removes a printer to be rid of its resin now has to delete the resin as well, and a
pool can fill with resins no machine uses.

`is_untouched` is a comparison against the stock profile, so a user who edits a new resin
back to exactly those numbers and that name loses it with the printer. That is the honest
bound of guessing intent from content; reopen this if a resin acquires a field that says
whether anybody has saved it.

## Alternatives considered

### Keep 0196's rule and warn harder

The question already named the loss. Making it louder does not give the file back, and the
screen has no undo to fall back on, so the only real protection is not deleting the file.

### Never delete a resin automatically

Simpler to state, and it was close. It loses to **New resin**: every abandoned click would
leave a profile in the pool that carries nothing measured, which is the state 0196 exists
to prevent.

### The option that won, and what it costs

Keeping the resin means the pool is now a place that needs tidying, and the rule has an
exception a reader has to learn. The exception is also content-derived rather than recorded,
so it is a guess — a precise one, but a guess.
