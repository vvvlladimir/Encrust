# 0046. Write `.ctb` layer data in the clear

- **Status:** Accepted
- **Date:** 2026-09-19

## Context

The `.ctb` header carries a 32-bit `encryption_key` at offset 0x64. When it is non-zero,
every layer's run-length data is scrambled before it is written: the key and the layer index
are folded by two multiply-adds into a 32-bit keystream word, whose bytes are XORed over the
layer and which advances by a constant every four bytes. Some vendor exports carry a
non-zero key; readers take both forms, and a key of zero means the bytes are
the runs themselves.

The firmware does not require a key. Files with a zero key print, which is what makes this
a choice rather than a constraint. `catibo` documents the same: the field is a cipher key,
and zero means no cipher.

There is a second, harder form — the encrypted `.ctb` variant with magic `0x12FD0107`,
whose whole header is wrapped in AES with a key that is not in any public document. That
one is a different format, not a flag, and it is out of scope here.

Two things pull against a key. The cipher is over the encoded bytes of a layer, so it runs
on the window being compressed, on every core, for no benefit to the print. And a file
nobody can read back is a file we cannot test against: `format-ctb` proves what it wrote by
decoding it, and the integration test in `encrust-cli` walks a real file's layer table and
expands every run.

## Decision

`format-ctb` writes `encryption_key = 0` and hands the run-length data to the file
unchanged. There is no option to set a key, and `CtbWriter` carries no field for one.

The codec in `rle.rs` is therefore symmetric: what `EncodedLayer::encode` produces is
exactly what `decode` reads, and both are exercised against each other.

## Consequences

A `.ctb` we write opens in the slicers that made the format, and a layer of it can be read back by
anything that knows the format, including our own tests and, later, a `.ctb` importer.

Every layer costs one pass fewer over its bytes.

Nothing is protected, which is the point of the field and not something this project wants.
A user who expects a slicer to hide their model from whoever holds the file will not get
that here.

Reopen this if a machine is found that refuses a zero key. The signal is a file that prints
from a vendor slicer and not from us with the same profile; the fix is a key on the writer and the
generator over each layer's bytes, which is about twenty lines, plus a test that decodes a
layer through it.

## Alternatives considered

### Write a random key, as the vendor slicer does

It is what the reference slicer does, and doing what the reference does is usually the
cheaper bet with an undocumented format. It loses because it buys nothing: the cipher is not
authentication and not compression, the firmware does not ask for it, and it would make
every file we write unverifiable by the tests that prove the format is right.

### Make the key an option on the writer

Zero by default, settable for a user who wants it. Rejected as a setting with no user: no
profile field, no CLI flag and no panel would drive it, and an option nothing sets is an
untested branch in the one place a mistake silently ruins a print.

### The option that won, and what it costs

Writing in the clear means our files differ from a vendor's in a field a reader can see, so
anyone comparing a vendor file against ours byte for byte finds the layer data unscrambled
and the key zero. If some machine or some future firmware treats a zero key as a marker of a
file it should not print, we find out from a failed print rather than from a decode error,
which is the worst way to find out.
