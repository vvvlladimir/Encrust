# 0047. Pick the sliced-file format from the output extension, in each binary

- **Status:** Accepted
- **Date:** 2026-09-19

## Context

Until now there was one sliced-file format. `encrust-cli` asked `is_goo(path)` and wrote
either a `.goo` file or a directory of PNGs; `encrust-app` always wrote `.goo` whatever the
save dialog came back with. With `format-ctb` beside `format-goo` there are two writers, and
something has to choose between them.

Three facts constrain where that choice can live.

`SlicedFileWriter` is generic in a way that cannot be made into a trait object:
`type Sink<'w>: LayerSink` is a generic associated type and `LayerSink::Encoded` differs per
format — `format_goo::EncodedLayer` carries a checksum that the `.ctb` one has no field for,
and each is compressed by its own codec. A `Box<dyn SlicedFileWriter>` is not expressible, so dispatch is a `match`
that ends in a generic function, once per format.

The dependency graph forbids a `core-*` crate from knowing a format crate:
`core-format` holds what the formats share and the format crates depend on it, never the
other way round. No existing crate can name both `GooWriter` and `CtbWriter`.

The two binaries do not share the streaming code the writer sits inside. `encrust-cli`
reports through `RasterReport` and returns errors with `anyhow` context; `encrust-app` reports
`Progress` to a channel a frame at a time and checks a cancellation flag between windows.
The `match` is four lines in each; what surrounds it is different code.

## Decision

The output path's extension names the format, case-insensitively: `.goo` is written by
`format-goo`, `.ctb` by `format-ctb`, and in `encrust-cli` anything else is the directory of
PNGs it has always been.

The extension cannot name a revision, so the revision comes from beside it: `--ctb-version`
in the CLI and the Slicing panel's format pill in the app, both defaulting to version 4.
The pill also supplies the extension the save dialog opens with, and the app appends it to a
name typed without one; a name typed *with* an extension keeps it, because that is what
decides the format.

Each binary owns its own `SlicedFormat` enum and its own dispatch: `encrust_cli::sliced_file`
and `encrust_app::job::format`. Each matches on it once, in a function generic over
`W: SlicedFileWriter`, and the streaming loop below that is generic over `S: LayerSink` so a
window is still compressed on every core through `S::encode`.

No new crate sits between the binaries and the format crates.

## Consequences

A third format is a new `format-*` crate, one arm in each binary's `match` and one entry in
its extension list. Nothing in `core-format` changes and no existing format is touched.

The app now fails on a name no writer claims — `plate.sliced` — instead of quietly writing
`.goo` bytes under it. The save dialog offers every extension and appends the chosen one to a
name without any, so the only way to reach that error is to type a foreign extension by hand,
and an error naming the path is better than a file the printer rejects.

Choosing `.ctb v5` in the panel and typing `plate.goo` writes a `.goo` file, and the pill
still reads `.ctb v5`. The name winning is the rule; the pill is a default, and the footer
line under the Slice button always states the name that will be written.

Two binaries carry the same six-line extension match, and a fourth format means editing both.
If a third front end ever appears, or if the two matches drift, that is the signal to pull
them into a crate that depends on every `format-*` crate and hands back a dispatcher.

## Alternatives considered

### A `formats` crate that depends on every format crate

One `SlicedFormat` for the whole workspace, one match, both binaries importing it. It is the
shape rule 5 of `AGENTS.md` asks for when two crates need the same type.
It lost on what it would actually hold: the type is an enum of two variants and a match on a
file extension, and the function around it — the one that opens the file, streams the stack
and reports — is different in each binary and would stay there. A crate whose whole content
is one enum, to avoid duplicating one match, is a crate boundary for the sake of a line
count.

### Choose the format in the printer profile

The machine's firmware is what decides which container it reads, so the profile is arguably
where the format belongs, and step 8b brings a profile catalogue that would carry it. It was
rejected for now because it makes the output name lie: a profile saying `ctb` and a user
typing `plate.goo` have to be reconciled, and there is no good answer. Revisit when 8b lands:
the profile can then supply the *default* extension the dialog opens with, while the name
still decides what is written.

### A trait object behind a boxed sink

Erase the format with `Box<dyn SlicedFileWriter>` and a sink of boxed encoded layers. It
would put the dispatch in one place at the cost of a `Box` per layer, an allocation per layer
on the hot path, and `Encoded` losing the type that makes `push` reject a layer of the wrong
panel size. It also does not work as stated without reshaping both traits, which exist in
their current form for streaming reasons that have not changed.

### The option that won, and what it costs

The extension deciding the format means a typo picks a format: `plate.gooo` in the CLI
silently becomes a directory of PNGs, because that is what "not a sliced file" has always
meant there. And the duplication is real — two enums, two matches, two lists of extensions,
in two crates that have to be edited together every time a format is added.
