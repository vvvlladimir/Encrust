# 0045. Write every sliced file to a seekable sink

- **Status:** Accepted
- **Date:** 2026-09-19

## Context

`docs/decisions/0012` settled how a sliced file is written: the header goes out first, the
layers stream through a `LayerSink` one at a time, and nothing but the window being
compressed is ever in memory. Its last line reads: *"The sink writes to `&mut dyn Write`.
It never seeks."* That held because the `.goo` container is laid out front to back — every
field is known before the byte in front of it is written.

`.ctb` is not laid out that way. Its header carries the offset of the layer definition
table, and each fixed-size record in that table carries the absolute offset of its own
run-length data. Neither number exists until the layers have been encoded, and the layers
come after both. Every implementation of the format writes placeholders and goes back over
them, and so does every converter built on `catibo`'s documentation.

Two ways out without seeking were on the table, and both cost more than seeking does:
holding the whole encoded stack in memory, which for a 70 mm sphere at 0.05 mm is 48.5 MB
and for a full plate is several hundred; or writing the layer data to a temporary file and
copying it in behind the finished header, which writes every byte of a large file twice.

The same constraint appears one more place: the `.goo` header's volume, weight and print
time are estimated from contour areas rather than measured from the written masks, precisely
because measuring them means summing coverage while streaming and going back over three
header fields.

## Decision

`SlicedFileWriter::begin` takes `&'w mut dyn WriteSeek`, where `WriteSeek` is the blanket
trait `Write + Seek` in `core-format`. Every format writer gets a seekable sink, whether it
uses the seek or not.

`core_format::Fields` gains `position` and `seek_to` beside the field writers, so a format
records where a placeholder went and fills it in later through the same type that wrote it.
`Fields::written` keeps its old meaning — bytes handed to the sink, counted again if they
overwrite earlier ones — because `.goo` asserts its fixed header size against it.

Everything else in 0012 stands: the header goes first, layers arrive one at a time through
`push`, `encode` stays a borrow-free associated function so a window can be compressed on
every core, and `finish` fails when fewer layers arrived than were promised.

## Consequences

`.ctb` becomes writable in one pass over the layers with no stack held in memory, which is
the whole point.

Output can no longer go to a pipe or to stdout. Nothing in the project ever wrote there:
`encrust-cli` and `encrust-app` both write a named file through a `BufWriter<File>`, which is
`Seek`. Tests that used a `Vec<u8>` now use a `Cursor<Vec<u8>>`, which is also `Seek` and
costs them one line each.

The estimated-totals shortcut is now a choice rather than a constraint. Measuring volume
and print time from the written stack and going back over the header is a change to
`format-goo` alone, and the reason not to do it today is that nobody has reported the
printer's estimate and ours disagreeing.

Reopen this if a format ever needs the file's bytes before they are a file — a network sink
that streams to a printer over LAN would be the signal, and the answer there is a sink that
buffers a bounded window rather than a change to this trait.

## Alternatives considered

### A second trait method for formats that need to seek

`begin` for streaming formats, `begin_seek` for the rest, chosen by an associated constant.
It keeps `.goo` honest about what it needs, and it puts the cost on every caller instead:
`encrust-cli` and `encrust-app` both dispatch on the output extension, and each would carry
two branches of otherwise identical code. The extra precision buys nothing a reader of
`format-goo` does not already get from reading it.

### Buffering the encoded stack in memory

The simplest change — no trait edit at all — and it fails at the size that matters. A full
plate is hundreds of megabytes, and 0010 and 0012 exist specifically so that peak memory
does not follow the model.

### The option that won, and what it costs

Requiring `Seek` of every format is a bound that `.goo` does not need and will never use,
so the trait now says something untrue about the simplest implementation of it. It also
rules out sinks that are genuinely write-only — a socket, a compressor, stdout — and if one
of those ever becomes the right way to send a file to a printer, this decision is in the
way and has to be reopened rather than extended.
