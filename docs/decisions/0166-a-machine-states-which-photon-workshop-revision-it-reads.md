# 0166. A machine states which Photon Workshop revision it reads

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

`format-anycubic` wrote one revision of the Photon Workshop container, version 1, which
three machines read. Every Anycubic machine sold since 2021 reads revision 516 or 517 of the
same container: the tables are in the same order, and a later revision inserts a grey table
in front of the layer table and four blocks between the layer table and the first layer's
runs.

The obvious shape was to let the extension imply the revision — `.pm5` is only ever read at
517. It does not hold: `.pwmb` is read at 516 by the Photon Mono X 6K and at 517 by the
Photon M3 Plus, and writing the wrong one to either is a file its firmware refuses.

An output file name also has the last word on which container is written (ADR 0047), and a
name cannot carry a revision.

## Decision

`OutputFormat::Anycubic` carries the extension **and** the revision, both stated by the
printer profile:

```toml
[output.anycubic]
extension = "pwmb"
revision = "v517"
```

`format-anycubic` writes version 1, 516 and 517. 515 is not written, because no published
machine profile asks for it, and 518 is not, because it adds a second preview and a
sub-layer table that two machines need.

Where an output name chose the container and the profile names the same one,
`SlicedFormat::at_revision_of` keeps the profile's revision: the name still decides the
family, and the profile decides the revision.

## Consequences

Fifteen more machines are in the catalogue and the picker, and the Anycubic line from the
Photon Mono X to the Mono M5 writes a file its firmware takes. The printer form's container
row shows the extension and not the revision, because a machine reads one of each and the
revision is not a choice a user makes.

The extension picked by hand in a file dialog still has to guess, and it guesses the newest
revision the extension's machines read. A user with a Mono X 6K who types `plate.pwmb` with
no profile behind it gets 517 and a refused file; a user who picks the machine does not.

Two things in the new blocks are inferences rather than transcriptions: the motion block's
stage counts, and the model block's bounding box. Both are written in
`docs/formats/anycubic.md` as such. The signal to reopen this is a machine that refuses a
file we write at the revision its own profile named.

## Alternatives considered

### Let the extension imply the revision

It would have kept the profile schema flat and the enum one field wide. `.pwmb` at two
revisions kills it, and there is no reason to expect it is the last extension to be reused.

### Write the newest revision every extension supports

One revision per extension, chosen as high as the reference implementation allows. It would
have written 517 to a Photon Mono X 6K whose own published profile asks for 516, which is
inventing a firmware capability rather than transcribing one.

### The option that won, and what it costs

A printer profile now carries a field only one container family uses, and a reader of the
schema has to know why. The two tables that keep extension and revision in step — the
generator's and `AnycubicFlavour::newest_version` — have to agree with each other, and
nothing but a test of the shipped catalogue holds them together.
