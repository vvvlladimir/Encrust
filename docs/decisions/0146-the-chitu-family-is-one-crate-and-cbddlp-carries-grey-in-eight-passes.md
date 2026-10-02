# 0146. Keep the Chitu family in one crate, and buy `.cbddlp` grey with eight passes

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

Step 18a adds `.cbddlp` and `.photon` output. Both were read against a reference file that
a reference writer produced, and the bytes settle two questions at once.

The first is where the code goes. `.cbddlp` is not a second format beside `.ctb`; it is the
same container at an older revision. Header, both preview records and the print parameters
block are byte-identical — the reference file's 112-byte header has the two padding words
at `0x14` and the total height at `0x1C`, exactly where `.ctb` version 4 has them, which
also settles that `catibo`'s document is the one off by four bytes. What the older revision
drops is the slicer info block, the version 4 block and the extended block in front of each
layer. What it changes is the layer codec and the table's size field. `.photon` differs from
`.cbddlp` in nothing but the extension a machine's firmware looks for.

The second is grey. `.ctb` carries seven bits of it in every run. The older container
carries one bit and gets grey only from writing a layer several times at rising thresholds,
which a machine sums back. ADR 0139 established that the full edge ramp is what makes a
file match one a machine is known to print, and measured the cost of losing it. At one pass
the only threshold is 255, so every anti-aliased edge pixel goes dark and the part loses
about a pixel on each side — the cliff ADR 0139 removed, reintroduced at the file boundary.

## Decision

Both members live in `format-chitu`, which is `format-ctb` renamed. `CtbWriter` and
`CbddlpWriter` are separate writers over shared `blocks`, `preview` and `layer` modules,
because `LayerSink::encode` takes no `self` and so cannot branch on a flavour at run time.
A `Head` struct carries the four fields that differ — magic, version, grey passes, slicer
info size — and one `write_header` serves both.

`.cbddlp` and `.photon` are written at version 2 with **eight passes**, the most the
container allows, giving the greys 0, 31, 63, 95, 127, 159, 191, 223, 255. The pass count is
a constant, not a setting.

## Consequences

A `.cbddlp` is eight times the layer data of a one-pass file, on top of a run-length floor
of one byte per 125 pixels whether lit or not. A 10 mm cube on a Mars 3 Pro comes to 134 MB.
That is the format, not our encoding: the machines it is for have panels a quarter that size
or less, and the alternative is a file whose edges are gone.

The grey the file round-trips is verified rather than argued: an independent reader decodes layer 100 of
our cube to our own mask quantised to those nine steps, with zero mismatches across all
10 490 880 pixels and the same 81 796 lit pixels, and reports no issues in the file.

Renaming the crate leaves ADRs 0046, 0047, 0048, 0127 and 0136 naming `format-ctb`. They are
committed and so frozen; this ADR is the pointer that the crate they mean is now
`format-chitu`.

The signal to reopen the pass count is a measured machine: one whose thin edges come out
liquid because a grey step landed below what it cures, or a user for whom the file size is
the thing that makes the format unusable. Either answer is a pass count on the profile, not
a new global constant.

## Alternatives considered

### A `format-cbddlp` crate beside `format-chitu`

The shape the architecture rules point at for a new format. It lost because the shared code
is the whole front of the file, and rule 5 would then push the header, the RGB15 preview
codec and the print parameters down into `core-format` — where they are not common to every
format but specific to one vendor. The readers that cover the family reach the same
conclusion, with one class for every extension in it.

### One pass, and no grey

Half a megabyte a layer instead of four, and what the vendor slicer itself writes when a user turns
anti-aliasing off. It lost on ADR 0139: a single pass thresholds at 255, so it does not
merely coarsen the edge, it deletes it.

### Four passes

Five grey levels for half the bytes. A real middle, and it was rejected only because there
is no evidence to prefer it: eight is what the reference writer produces, so it is the one
whose output we can check byte for byte against a decoder that machines agree with.

### The option that won, and what it costs

Eight passes make the format's own floor dominate the file. A large model on a 4K panel will
produce a `.cbddlp` measured in gigabytes, and nothing in the writer warns about it — the
size is only visible once the file is on disk.
