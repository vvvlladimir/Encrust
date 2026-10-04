# 0176. The command line is `encrust`, with subcommands, JSON and a clean Ctrl-C

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

The CLI was one binary, `slice`, with about forty flags on one command; modes were flags
(`--list-profiles`, `--read`, `--no-slice`, `--no-raster`) and a directory as input switched
to a batch run. Only a batch wrote JSON, the one exit code for trouble was 1, logs went to
stdout beside the report, and Ctrl-C killed the process mid-write, leaving half a file. The
window was the binary called `encrust`. The release archive held both as `encrust/encrust`
and `encrust/slice`, and the window's updater (ADR 0172) swaps them by those names.

## Decision

The command line is the binary `encrust`; the window is `encrust-gui`. `encrust` takes a
subcommand — `slice`, `batch`, `inspect`, `info`, `profiles list` — each with its own
`--help`, built with clap's derive over flag groups the subcommands share.

`--json` on every subcommand prints exactly one document to stdout carrying `"schema": 1`;
logs and progress always go to stderr. A failure under `--json` is a document too,
`{"error": {...}}`. `slice`, `inspect` and each model of a batch share one report type.
Exit codes: 0 success, 1 failure, 2 bad arguments, 3 `--strict` and a model with defects,
4 a batch in which a model failed, 130 stopped by Ctrl-C.

The first Ctrl-C sets a flag that `core_pipeline::Observer::cancelled` reads, so the run
stops between layers and the writer removes its file; a second exits at once. The flag is
installed with `ctrlc`. A layer bar is drawn with `indicatif`, only on a terminal and never
under `--json`, `--quiet` or `--no-progress`.

The release archive's directory becomes `Encrust/`, holding `encrust-gui` and `encrust`.

## Consequences

A script reads one JSON shape whatever it ran, and gates on the exit code without parsing
anything. The binary's name matches the product, and later commands (`estimate`, `convert`,
`printer`) have somewhere to go.

Every old invocation breaks; at 0.1 nothing is kept for compatibility. A window from before
this decision looks for `encrust/encrust` in the archive, finds nothing, and sends the user to
the release page instead of installing the CLI over itself: the directory was renamed for
exactly that. An installed shortcut to the old `encrust` binary now starts the CLI until the
installer rewrites it.

Two dependencies enter `encrust-cli`, and only there. Reopen if a later step drops the bar
or wants signals beyond Ctrl-C.

## Alternatives considered

### Keep `slice` as the name

No change to packaging or the updater, but `slice slice model.stl` reads badly, and every
later subcommand would live under a verb.

### `encrust-cli` beside `encrust` for the window

Leaves the window and the updater alone, but the name a user types every day would be the
longer one.

### Ctrl-C through `signal-hook`, or a bar printed with `\r`

`signal-hook` covers Unix only, and Windows would need its own code. A hand-drawn bar needs
its own rate limit, ETA and terminal width, all of which `indicatif` already has.

### The option that won, and what it costs

Renaming the window touches the release workflow, the packager's binary list, the updater
and every doc that starts either binary, and leaves a pre-rename install one manual
download behind. `indicatif` brings `console`, `unit-prefix` and `portable-atomic` with it.
