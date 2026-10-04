# Fuzzing the readers

Every byte Encrust parses was written by somebody else: a mesh from a model site, a sliced file from
another slicer, a texture from a pack. The property under test is not "it works" but "it answers" —
no panic, no hang, no allocation proportional to a number in a header, whatever the bytes are. That
is also what [`SECURITY.md`](../SECURITY.md) promises, and this is how the promise is checked.

The crate is outside the workspace on purpose. `cargo fuzz` builds with `-Z` flags that need
nightly, and the product stays on the toolchain pinned in `rust-toolchain.toml`. See
[ADR-0161](../docs/decisions/0161-the-readers-are-fuzzed-outside-the-workspace.md).

```sh
cargo install cargo-fuzz
cargo +nightly fuzz list
cargo +nightly fuzz run sliced_chitu -- -max_total_time=60
cargo +nightly fuzz run mesh         -- -max_total_time=300 -max_len=65536
```

Ten targets:

| target | what it covers |
| --- | --- |
| `mesh` | STL, OBJ and 3MF through `loader_for_extension`, and `Texture::decode` behind them |
| `sliced_chitu` | `.ctb` v4 and v5, `.cbddlp`, `.photon`, and RLE7 and RLE1 |
| `sliced_goo` | the `.goo` header, layer table and run-length encoding |
| `sliced_anycubic` | `.pwmx` and the six extensions beside it, and PW0 |
| `sliced_sl1` | `.sl1` and `.sl1s` — a zip of PNGs, so the zip and PNG decoders with it |
| `sliced_gcode_zip` | the `.zip` whose `run.gcode` is the program, parsed line by line |
| `sliced_creality` | `.cxdlp` at versions 3 and 4: the vertical lines, the tables and RLE7 |
| `sliced_svgx` | `.svgx`: the header, the document index and the path data of one group |
| `sliced_cws` | `.cws`: a zip, `slice.conf`, the gcode program and the PNGs behind it |
| `sniff` | a file whose name says nothing: the sniffer picks the family, possibly the wrong one |

Each reader is given the extension it expects rather than left to be found by chance, because a
fuzzer spending its session failing a magic-number check tests the magic-number check.

## The corpus

`corpus/<target>/` is the seed, and it is worth keeping small and *varied* rather than large: one
file per shape the reader branches on.

`corpus/mesh/` is seeded already, from `core-mesh-io`'s own test fixtures. Each carries the selector
byte the target reads first — `0` for `.stl`, `1` for `.obj`, `2` for `.3mf` — so adding another goes
through `printf`:

```sh
printf '\x00' | cat - ../crates/core-mesh-io/tests/fixtures/tetrahedron.stl > corpus/mesh/tetrahedron
```

For the sliced containers there is nothing checked in, because the writer tests build their fixtures
in memory. Seed them from the CLI instead, one small file per format. Slice on the smallest panel in
the catalogue and a thick layer: a seed off a 16K machine is sixty megabytes, which is a file the
fuzzer spends its session copying rather than mutating.

```sh
cd .. && mkdir -p /tmp/seed && for fmt in goo ctb cbddlp sl1 zip svgx cws pwmx; do
  cargo run --release -p encrust-cli --bin encrust -- slice crates/encrust-app/tests/fixtures/cube.stl \
    --printer anycubic-photon-zero --resin standard-grey --center --layer-height 0.5 \
    -o "/tmp/seed/cube.$fmt"
done
```

`.cxdlp` needs a machine that writes one — `--printer creality-halot-lite-cl-89l`. Then move each
into the matching `corpus/<target>/`, and give `sniff` one of every container. A `.goo` stays over
two hundred kilobytes whatever the panel, because its header alone is 195 of them, so a run that is
to reach its layer walk needs `-max_len` raised past that.

## When something crashes

**A crash is committed.** Copy the reproducing input into the owning crate's
`tests/fixtures/crashes/`, add the case to that crate's tests, then fix it. The test is what keeps
it fixed; the corpus file is what keeps the fuzzer from having to find it again.

CI runs a short session per target on a schedule
([`.github/workflows/fuzz.yml`](../.github/workflows/fuzz.yml)), never as a gate on a pull request:
two minutes of fuzzing is a smoke test, and failing a pull request over which inputs one random
session happened to reach would teach everybody to ignore it.
