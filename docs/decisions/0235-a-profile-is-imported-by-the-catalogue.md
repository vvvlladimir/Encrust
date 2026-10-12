# 0235. Import and export a profile through the catalogue

- **Status:** Accepted
- **Date:** 2026-10-12

## Context

ADR 0233 carries a profile out of the catalogue as its TOML and takes one back in under an
id of its own, and put that work in `encrust-app`'s `exchange.rs`. The command line lists
and shows the catalogue too, and a user moving a measured resin between computers is as
likely to do it from a script. Two front ends needing one piece of work moves it down
(rule 5); a copy in each would drift on the one thing that matters, never writing over
what the user has.

## Decision

`printer-profiles` owns the exchange: `Catalogue::export(kind, id)`,
`Catalogue::import(kind, text, path, printer)` for printers, resins and support profiles,
and `Catalogue::import_preset::<T>(kind, text, path)` for a tool's preset, held to reading
as `T`. The crate stores presets without knowing any tool's settings (ADR 0226), so the
caller names the type: `HollowState` or `DrainState`. `unique_id`, the file stem a name
makes, moves down with it. The window's `exchange.rs` keeps the status bar and the choice
of type; `encrust profiles import FILE --kind KIND [--printer ID]` is the same call.

## Consequences

The window and the command line import the same way, and the rules of an import — a
" (imported)" name, a fresh id, a resin set up on the machine it came in on — are tested once,
in the crate that keeps the files. `import` refuses a preset kind with `Unknown`, as
`save_preset` refuses a profile kind, so a caller that forgets the type is told rather than
storing settings nothing has checked.

## Alternatives considered

### Keep the work in the window and repeat it in the command line

Fifty lines twice, and the first change to how a name is made would happen in one of them.

### Move it down, and what it costs

`printer-profiles` grows a second way in besides `save_*`, and its import of a preset takes
a type parameter only its callers can fill in.
