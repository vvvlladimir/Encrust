# 0395. Read 3MF with `quick-xml` and the workspace's `zip`

- **Status:** Proposed
- **Date:** 2026-11-03

## Context

ADR 0112 took `threemf2` to parse 3MF. Its newest release, 0.4.1, still needs `zip` ^8.6, and
every `zip` below 9.0.5 carries advisories: one critical, a parser differential in plain
reading, and others in `extract`, AES and the legacy decoders. `cargo deny` failed on each new
one, and ten security alerts stayed open on a crate the workspace had already left behind
for `zip` 9. What the loader reads of 3MF is small: the root model part, parts named by the
production extension's `p:path`, objects, meshes, components, the build, the unit, and the
materials extension's `texture2d` and `texture2dgroup`.

## Decision

`core-mesh-io` reads 3MF itself, through the workspace's `zip` 9 and `quick-xml` 0.41, the
version already in the tree. `threemf2` leaves the workspace.

- `three_mf/package.rs` opens the zip, refuses a part claiming more than it could inflate to
  before reading any, finds the root through the `StartPart` relationship in `_rels/.rels` or,
  without one, at `3D/3dmodel.model`, and looks a part up as written or percent-decoded,
  case-insensitively. An image part that fails to read is `Malformed`, not left unmapped.
- `three_mf/parse.rs` reads a model part in one streaming pass into `three_mf/model.rs`: no
  document tree, vertices as `f32` once. An element of a namespace it does not read is skipped
  with all it holds; of two resources sharing an id the first is kept, as the walk took it.
- The walk is ADR 0112's, now over these types. Two rules of the specifications are kept where
  `threemf2` left them to us: a triangle without `p2` or `p3` takes `p1` for every corner, and
  without `p1` its object's `pindex`; a component without `p:path` places an object of its own
  part, not the root's; one outside the root naming another part is `Malformed`. A non-finite
  number is `Malformed`.
- `zip` 9 takes `deflate-flate2-zlib-rs`, the `flate2` backend the tree had only through `zip`
  8, so no deflate elsewhere changes speed.

## Consequences

The tree loses `zip` 8, `threemf2`, `instant-xml`, `lexical-core` and `compact_str`. Loading
200 000 triangles of 3MF goes from 87 to 84 ms (`load 200k triangles from 3mf`), and only the
image parts a face names are read into memory, not every part of the package.

What is now ours to keep: three namespaces, the attributes of a dozen elements, and OPC part
naming. `requiredextensions` is not enforced, as before: a file requiring an extension we do
not read loads what core it carries, where the specification says to refuse it. A file using
slices, beam lattices or another extension we skip is the signal to revisit that.

## Alternatives considered

### Ignore the advisories until `threemf2` moves to `zip` 9

Reading never calls `extract`, and the build compiles neither AES nor the legacy decoders. The
critical differential is in plain reading, though, and every new `zip` 8 advisory breaks the
audit again with no date for an upstream fix.

### Patch `threemf2` onto `zip` 9 through `[patch.crates-io]`

Keeps the parser, at the price of a fork to carry, a git source in `deny.toml`, and the same
crate's young surface ADR 0112 already worked around.

### Reading it ourselves, as chosen

The specification is ours to re-read where a file shows we got it wrong; ADR 0112 rejected
this for that reason. The surface turned out to be one streaming pass over a dozen elements.
