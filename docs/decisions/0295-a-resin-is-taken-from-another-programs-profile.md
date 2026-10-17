# 0295. Take a resin from another program's exported profile

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

ADR 0293 takes a resin from a sliced file, which holds what the machine needs and nothing
else. The profile another program exports holds the rest: colour, density, price and every
compensation the user measured there. Two formats are open: the `.cfgx` machine
configuration, which is JSON, and the `key = value` material INI. Neither is TOML, so the
importer of ADR 0235 cannot read them, and the bounds a sliced file's values pass through
(ADR 0293) sat in `core-format`, which `printer-profiles` sits under.

## Decision

`printer-profiles` reads both in a `foreign` module: `ForeignResin::from_str` answers the
resin on our defaults wherever the file has no place for a setting, with the machine and
panel the file names, and `Catalogue::printer_for_foreign` finds that machine as
`printer_for_panel` does, or by name alone where no panel is stated. The format is told by
the extension. `printer-profiles` takes `serde_json`, already in the workspace, for the
`.cfgx`. The bounds of ADR 0293 move down into `printer-profiles`; `core-format` re-exports
them, so a `format-*` crate still reaches them through `core-format` alone (ADR 0148).

`encrust profiles import FILE` with no `--kind` reads a `.cfgx` or `.ini` this way, and the
window's resin imports take both. Where each value comes from is in
`docs/formats/resin-profiles.md`.

## Consequences

A user moving over brings their resin with its colour, price and compensation, set up on
the right machine. An encrypted profile, and a configuration bundle holding several
materials, are not read; a bundle merges its sections and the last material wins. Reopen if
a third open format appears that is neither JSON nor INI, or a bundle turns out to be what
users export.

## Alternatives considered

### A crate of its own above `printer-profiles`

It would keep JSON out of the leaf, but it reads into one type and calls one method of the
catalogue; a crate would be one more edge for a module's worth of code.

### Our own small JSON reader

No new edge, but a parser of somebody else's bytes to keep correct and fuzz, where a
well-fuzzed one is already in the lock file.

### The option that won, and what it costs

The leaf crate now builds `serde_json`, and its price unit for a `.cfgx` rests on one
observed file: the field's own list reads litre first, the window showed kilogram for 0.
