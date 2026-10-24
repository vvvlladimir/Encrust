# 0337. A shipped support preset lives only in its file

- **Status:** Accepted
- **Date:** 2026-10-23

## Context

ADR 0132 kept every shipped support preset twice, as its TOML in
`assets/profiles/supports/` and as `SupportProfile::light()`, `medium()` and `heavy()`, with a
test failing whenever the two disagreed. Retuning a preset meant editing two files, and every
other preset the catalogue ships — the automatic shapes among them — had no constructor at
all. The constructors existed so that a binary or a test could name a preset without a
catalogue on disk; the files are compiled in either way.

## Decision

**The file is the preset, and the only copy.** The constructors are gone.
`SupportProfile::shipped(id)` reads the preset this build ships under `id` from the embedded
file, `None` for one it does not ship; a test reads every shipped file and checks it is valid.

**Every field has a default of its own**, so a profile file that leaves one out still reads,
and `SupportProfile::default()` is the shipped [`STOCK_SUPPORT`], `medium`, as its file says,
falling back to those defaults only if that file did not read. The defaults are what a missing
field means, not a preset: nothing ships them.

## Consequences

A preset is retuned by editing its TOML and nothing else. A test that uses a shipped preset as
its input reads it from the file, so retuning one may move a number such a test pins; a test
that needs fixed numbers builds its own profile. The parity test is deleted.

## Alternatives considered

### Keep the constructors and the parity test (ADR 0132)

Rejected: two copies of every number, and only three of the shipped presets covered.

### Remove `Default` altogether

Every place that needs a profile before a catalogue is read — a new window's tools, a project
with no groups — would grow a `Result` for a file that is compiled in and tested.

### The option that won, and what it costs

The field defaults are a third set of numbers in the code, used only when a file leaves a
field out or the stock file fails to read; a shipped file that does not read is caught by a
test rather than by the compiler.
