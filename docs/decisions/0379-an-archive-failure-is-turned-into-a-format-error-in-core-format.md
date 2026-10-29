# 0379. Turn an archive's failure into a `FormatError` in `core-format`

- **Status:** Accepted
- **Date:** 2026-10-29

## Context

Four format crates write a zip archive — `format-sl1`, `format-gcode-zip`, `format-cws` and
`format-anycubic`'s PWSZ — and three of them read one. Each carried its own copy of the same
mapping from `zip`'s error to `FormatError`: its I/O as `Io`, anything else as a broken
archive. Seven copies of one rule break `.claude/rules/code-style.md`'s "a second copy is
extracted into the lowest crate owning the concept", and the lowest crate every format
shares is `core-format`, which did not depend on `zip`.

## Decision

`core-format` takes `zip`, already in the workspace with the same features, and implements
`From<zip::result::ZipError> for FormatError`. A format crate converts with `?`. The `.cws`
reader keeps its own mapping, because a broken archive there means "not this format".

## Consequences

One rule, one test, and a new archive format inherits it. Every crate on `core-format`
now builds `zip`; every writer of an archive took it already, and the rest share the
workspace's one build. Reopen if `core-format` must build without `zip`, for the web
target or a format crate that cannot afford it.

## Alternatives considered

### A crate of its own for archive helpers

Keeps `core-format` free of `zip`, at the cost of a crate holding one conversion and an
edge from four format crates to it.

### The option that won, and what it costs

`core-format` depends on a container library that the binary formats never use.
