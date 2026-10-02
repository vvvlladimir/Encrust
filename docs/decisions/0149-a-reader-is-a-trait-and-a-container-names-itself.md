# 0149. Make a reader a trait, and let a container name itself

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

Steps 18a to 18c left four writers and no way to open what they wrote. The step-18
milestone is that a file from another slicer opens here, which needs a container parser for
each family, something to choose between them, and a caller.

Three things shaped it.

`core-format` already has the writing half: `SlicedFileWriter` hands back a `LayerSink` that
takes layers one at a time, so no stack is ever held whole (ADR 0010). Reading has the same
constraint from the other side — a reader must not hold every mask — and the same need for a
seekable source, because three of the four containers address a table from a header.

Rule 2 says to extend through traits only when the second implementation actually arrives.
Here four arrive at once, and `encrust-cli` needs one path through them.

And a reader cannot assume the layer data is in the clear. A `.ctb` from the slicers that
made the format covers each layer's runs with a cipher keyed in the header, and one of them
sets a random key for every `.ctb` while always clearing it for `.cbddlp`. ADR 0046 decided we *write*
a zero key; it said nothing about reading, and a reader that ignored the field would decode
noise from almost every real `.ctb`.

## Decision

`core-format` gains the mirror of its writing half: `SlicedFileReader::open` returns an
`OpenFile`, which reports a `SlicedFile` of what the container says and decodes one layer at
a time on demand. `Reads` is the dual of `Fields`, and `ReadSeek` of `WriteSeek`.

**`SlicedFile` reports what the file says, not what we would have written.** A field a
container has no place for is `None`, and that is itself worth showing: it is why two files
of one stack differ. `LayerEntry` is held per layer, because the table is how a reader
reaches a layer at all; it is the smallest thing that can be, and no mask is held.

**A container is recognised by its first twelve bytes** when the name does not say. Every
family announces itself there — `ANYCUBIC`, `PK`, the `.goo` tag at offset 4, a Chitu magic
word — so one read settles it and no reader is ever run speculatively on a file that is not
its own. The extension still wins when there is one (ADR 0047).

**`core_pipeline::Opened` is an enum over the four, not a boxed trait object.** All four are
in this workspace, and a fifth would arrive as a `format-*` crate beside them.

**`format-chitu` implements the `.ctb` layer cipher.** It is symmetric, so one function both
directions. Our writer still writes a zero key; ADR 0046 stands.

## Consequences

`encrust-cli --read FILE` opens any container this project writes, whoever wrote it, prints
what the file states and then decodes every layer — because a header can be read from a file
no machine would print, and the layers are where a container is really tested.

Verified against independent readers and vendor output: all six reference files open, every
layer decodes, and every layer covers exactly its panel. One source model in six containers
gives a lit-pixel count that falls with each container's grey depth and nowhere else —
5 267 914 for `.goo`, `.sl1` and `.sl1s` alike, 5 262 190 for a seven-bit `.ctb`, 5 241 477
for four-bit PW0, 5 226 379 for nine-step `.cbddlp`. Three independently written parsers
agreeing to the pixel on the lossless formats is the evidence that they are right.

It also shows up a thing worth showing: `demo.goo` states 1317 mm³ of resin and its masks
cure 1317 mm³, while the `.ctb` and `.cbddlp` of the same model state 763 and 898 against
the same masks. A reader recomputes that field rather than trusting it, and our report
prints both numbers so the disagreement is visible instead of hidden.

What this does not do is open the container **versions** we do not write: a `.ctb` at version
2 or 3, an Anycubic file at 515 or later, an encrypted `.ctb` under its own magic. Each is
named in the error rather than half-read.

The signal to replace the `Opened` enum with a trait object is a reader that does not live in
this workspace, which would mean a plugin, which rule 3 rules out.

## Alternatives considered

### Try each reader in turn until one succeeds

The obvious way to handle a file with no extension, and what was written first. It lost on
two counts: the borrow of the source cannot outlive a failed attempt without contortions, and
running a parser on bytes that are not its own is how a reader ends up reporting a plausible
number from a file it misread.

### Read the whole layer table into masks at open

Simpler to use, and it makes `layer()` infallible. It loses on rule 7 and ADR 0010: a stack
is hundreds of megabytes, and the reason to open a file is often to look at one layer of it.

### Refuse a `.ctb` whose key is set, and say so

Consistent with ADR 0046 read narrowly, and it would have been honest. It lost because it
refuses nearly every `.ctb` in existence, which makes the milestone unreachable: the cipher
is fifteen lines and covers only the runs.

### The option that won, and what it costs

`SlicedFile` is one shape for four containers that do not agree on what they record, so it is
a lowest common denominator with holes in it. A `.ctb`'s bed size, an Anycubic file's
per-layer lift, a `.sl1`'s hundred settings keys — none of them survive into it, and a
user who wants them still has to reach for a dedicated reader.
