# 0376. A sliced-file sink can be read back

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

By ADR 0045 every format writes into `&mut dyn WriteSeek`, `Write + Seek`: a header that
points at a table it precedes is filled in by going back over it. A trailer over the bytes
before it was so far computed while they streamed past, as the CRC that ends a `.ctb` is,
because a CRC of two runs of bytes combines from the CRC of each.

The `.goo` revision 5.1 the vendor writes for the Jupiter 2 ends with the MD5 of every byte
before it. Its tables at the front state where each layer's masks lie and how long they
are, so they are final only after the last layer; the masks follow the tables. An MD5 does
not combine, so it can only be taken over the finished bytes in file order. Two front ends
handed the writers a sink that could not read: `BufWriter<File>` on a desk, and a
`BufWriter` over the browser's private-storage file in a worker.

## Decision

The sink trait is `ReadWriteSeek`, `Read + Write + Seek`, renamed from `WriteSeek`, in
`core-format`. `Fields::read_back` hands every byte from the start to the end of the sink to
a closure a megabyte at a time and leaves the position at the end, where the trailer goes.

`core_format::BufferedSink` is the buffered file that can still be read: a `BufWriter` that
flushes before a read. `core-pipeline` opens its output read and write and wraps it in one;
the browser's stored file reads at its offset through the same synchronous handle it writes
with, and is buffered the same way.

## Consequences

A format may now finish with a hash of the whole file, at the cost of reading the file once
more; only the `.goo` 5.1 writer does. Memory stays one chunk whatever the file's size.

Every sink a caller passes must be readable. A `Cursor<Vec<u8>>` and a `File` opened read and
write are; a `File::create` handle or a plain `BufWriter` no longer compile in that place,
which is where a caller learns of it rather than from a failed write.

## Alternatives considered

### Spill the masks to a temporary file

The `.goo` writer would hold its layers in a file of its own and lay the container down in
order at the end, hashing as it goes. It leaves the trait alone, but a format crate starts
creating files, and every byte of a large file is written twice.

### Leave the MD5 out

Whether the firmware checks it is not known, and a file it refuses is found only at the
machine.

### The option that won, and what it costs

Every front end's sink changed type, and a `.goo` 5.1 file is read once after it is
written: on a slow stick that read is not free.
