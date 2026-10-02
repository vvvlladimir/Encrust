# 0150. Let an open file own the source it reads from

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

ADR 0149 gave every container a reader whose `OpenFile` decodes one layer at a time on
demand, which is what keeps a reader from holding a stack. It took the source as
`&'r mut dyn ReadSeek`, and the open file borrowed it for `'r`.

That is right for `encrust-cli --read`, which opens a file, walks it once and drops both
together. It does not work for a window. `encrust-app` has to hold the open file across
frames, because the point of opening one is to move a slider through it; holding the reader
and the `BufReader` it borrows in one struct is self-referential, and the only ways out are a
crate like `ouroboros` for a single call site, or reopening the file on every slider tick —
which for a `.sl1` means reparsing a zip's central directory each time.

## Decision

A reader is generic over its source rather than taking a trait object reference:

```rust
type Open<S: ReadSeek>: OpenFile;
fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError>;
```

`Reads<S>` owns its source, and so does every `OpenFile`. The lifetime is gone.

Both callers fall out of it, because `&mut File` and `Box<dyn ReadSeek>` are each a
`ReadSeek`:

- `open(path, &mut buffered)` reads the tables and hands the file back, which is what the
  CLI and the tests do.
- `core_pipeline::open_file(path)` returns an `Opened<Box<dyn ReadSeek>>` that keeps the file
  open for as long as layers are wanted, which is what a window holds.

`open_path`, which returned the facts and the reader as two separate values, is gone: it
existed only because the borrow could not be kept.

## Consequences

No new dependency, no self-referential struct, and no reopening. The change is mechanical —
four readers, `Reads`, and the `Opened` enum — and every existing test passed unaltered
afterwards, which is the evidence that nothing but the plumbing moved.

`Opened` is now generic, so a caller has to name its source type. For the window that is
`Opened<Box<dyn ReadSeek>>`, which is a mouthful; the alternative was a lifetime it could not
satisfy at all.

A boxed source costs one indirection per field read. Reads go through a `BufReader` either
way, so the cost lands on a buffer hit rather than a syscall.

The signal to revisit is a reader that has to be `Send` — moving a stack decode to another
thread would need `Box<dyn ReadSeek + Send>`, which is a bound to add when something asks
for it and not before.

## Alternatives considered

### Add `open_owned` beside `open`, taking `Box<dyn ReadSeek>`

The smallest diff, and what was first proposed. It lost because it needs a second associated
type and a second implementation per reader for what is one operation over two source kinds
— which is exactly what a generic parameter expresses.

### Hold the reader and its source in a self-referential struct

Keeps the signature. It costs a dependency whose only job is to work around a borrow, against
rule 3, and it puts the awkwardness in the window rather than in the layer that caused it.

### Reopen the file for every layer the slider lands on

No API change at all. It lost on a zip: an `.sl1`'s central directory would be parsed again
for every tick of the slider, and a file on a slow disk would make the slider stutter.

### The option that won, and what it costs

Generics leak into the signature of everything that holds an open file, where a trait object
would have stayed invisible. `Opened<Box<dyn ReadSeek>>` appears in the window's state, and a
future caller that wants to store an open file in a struct has to name the source type there
too.
