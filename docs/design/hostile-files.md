# Reading a file somebody else wrote

A header is a claim, not a fact. Every number in one — a layer count, a table address, a
panel, a run length, the uncompressed size of a zip entry — arrives from whoever made the
file, and a reader that sizes a buffer from one of them has handed that person the
allocator. The containers in `format-*` are read under this assumption; why it is settled
in `core-format` rather than in each reader is [ADR 0171](../decisions/0171-a-count-read-from-a-file-is-checked-against-the-file.md).

## The three guards

**A count is checked against the source.** `Reads::claim(what, count, each_bytes)` asks
whether the file is even large enough to hold what the header claims, and answers the
capacity to reserve. `each_bytes` is the *least* one of the claimed things costs: a layer
table row is its fixed width, a `.goo` layer is its definition plus the two bytes its size
field counts beside the runs. The product is a checked multiplication, so a count near
`u64::MAX` is refused rather than wrapped. `Reads::bytes` applies the same test to a single
length field before it allocates.

This bound is loose on purpose. It measures against the whole file rather than against the
bytes behind the table, because that is what one call can know without the caller seeking
first. It does not have to be tight: it has to turn an unbounded allocation into a bounded
one, and the per-row reads behind it fail on their own offsets anyway.

**A panel is checked against physics.** No count in a file bounds `width_px * height_px`:
a mask is a mask whatever compressed the data. `panel_in_range` refuses the overflow and
anything past `MAX_PANEL_PX`. The number is 256 megapixels — roughly three times the
largest panel sold, so no printer is refused and no header can ask for a mask the size of
memory. A PNG's own `IHDR` goes through the same check, because in an archive container
that is where the panel comes from.

**An archive entry is grown, not reserved.** A zip states each entry's uncompressed size in
its own directory, and the obvious reader allocates that much before decompressing a byte.
`read_entry` reads with a cap instead and refuses anything past `MAX_ENTRY_BYTES`. The
3MF loader cannot do this — the reader underneath it owns the zip — so it checks the
directory itself and refuses the archive before handing it over.

## Where this is proven

Nine fuzz targets in `fuzz/`, one per container plus the sniffer, and the mesh loaders.
A crash becomes a fixture and a test in the crate that owned it; see
[`fuzz/README.md`](../../fuzz/README.md). The property is not "it works" but "it answers":
no panic, no hang, and no allocation proportional to a number in a header.
