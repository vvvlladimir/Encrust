# 0481. Check a job with no machine on the standard one, and shape cut pieces after the cut

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

A run of `encrust-mcp` against the user's own model, through the window and from three chat
sessions, made the same plate point mean two places on the plate. The run is in
`local-docs/AUDIT-mcp.md`. By ADR 0475 a job the user has named no machine for is checked with
no machine flag at all. `--center` with no printer centres the model on the origin, and the
window's plate of the job keeps that place. Once the user names a printer, the command line
centres the model on that printer's plate, 109.44 mm on for the standard printer. A cut at
`x: 0` then landed on the model on one path and missed it on the other. The client could not
tell which path a revision would take, so it gave up and started a new job. No report said where a model
stood, so a client guessed the plate's middle.

The same runs showed three more defects behind the agent's retries:
- A cut laid its halves on their faces after the pose search and the lift had run on the
  whole model. Both were lost, and the report gave the one search summary to the first piece.
- An orient handed to `--edits` named the user's own resin, which the shipped catalogue the
  document proves on never lists.
- Supports asked for one piece replaced those asked for every piece.

## Decision

- **`--standard-machine`** checks on `STANDARD_PRINTER` with the stock resin numbers. The
  model is centred and arranged on that plate, and a project is staged on it. `slice`,
  `batch` and `convert` refuse the flag. `encrust-mcp` passes it for every job of a model
  the user has named no machine for, and `write_file` still asks for theirs (ADR 0475).
- **Each check reports `models[].fit.bounds_mm`**, the model's low and high corners on the
  plate. The server's instructions tell a client to take every plate point from there.
- **Where the modelling flags make pieces** (a cut, a split, a part off), the pose search, the
  turn after it and the lift run on each piece after the modelling. Each piece's search
  summary is reported on that piece.
- **An orient read from `--edits`** is proved as `--orient` proves it: on the printer the
  shipped catalogue lists, the standard one for the user's own, and with the stock resin
  numbers.
- **Supports asked for some objects** stand over those asked for every one, kept beside them
  in the revision as `more_supports`, and run after them by `--only` (ADR 0479).
- **A revision after a cut the window holds** is shown on the window as the plate its check
  wrote. No edit of the job's model reaches the pieces.

## Consequences

A plate point keeps its place from the job's first revision to its file, as long as the
user's printer has the standard plate. On a printer with another plate, the model is
centred again; `bounds_mm` gives the new place, but points given earlier are not moved.
Without a machine, the check prints a time and a resin figure from the stock numbers. This
is what 0475 already accepted for a pose.

A turn or lift together with a cut now shapes the pieces, where it used to shape the model before the
cut. A run that relied on the old order turns out differently. A change to the lift still runs the
pose search again, and a search can rank another pose first. Reopen if a client needs a pose
to stay as it was picked.

## Alternatives considered

### Translate the plate points when the machine changes

The server moves a cut and its points by the difference between the two plates' middles.
This keeps them on a printer of another size, but it does not cover the window's plate,
which it does not hold. The two paths would still disagree.

### Make the command line centre on the standard printer with no flag

This needs no new flag. But a project and an `estimate` with no machine would then check
silently on a printer the user never named.

### The option that won, and what it costs

The command line gains one flag, and three writers must refuse it. A job with no machine
reports print figures from the stock numbers, and a client may read them as the user's.
