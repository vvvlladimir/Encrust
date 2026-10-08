# 0209. A front end names a container through core-pipeline

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

`core_pipeline::SlicedFormat` is what both front ends offer on a picker and what chooses
the writer (ADR 0047, 0127). Its variants carry the revision or extension a family needs —
`Ctb(CtbVersion)`, `Cbddlp(CbddlpFlavour)`, `Anycubic(AnycubicFlavour, AnycubicVersion)`,
`Sl1(Sl1Flavour)`, `Cxdlp(CxdlpVersion)` — and those types are defined in the crate that
writes each container.

So naming one meant depending on the crate: `encrust-app` listed seven `format-*` crates
and `encrust-cli` four, for four type names between them and nothing else. Three of the
app's and two of the command line's were not referenced at all. No front end calls a
writer: `core-pipeline` does, and has since ADR 0127.

## Decision

`core-pipeline` re-exports the flavour types its own variants carry, the way `core-format`
re-exports `LayerPlan` so a `format-*` crate gets it without a dependency (ADR 0091). A
front end names a container through `core_pipeline::CtbVersion` and takes no edge to
`format-chitu`.

Neither binary depends on a `format-*` crate any more. `encrust-app` and `encrust-cli` each
keep `format-goo` as a dev-dependency, for the one test apiece that reads back a file it
asked the pipeline to write.

## Consequences

Eleven edges leave the graph, and the rule that a container's crate is reached through the
stage that writes it now holds without exception — a new format is a `format-*` crate plus
a `SlicedFormat` variant, and no front end changes to see it.

`core-pipeline`'s public surface grows by six type names that are not its own. They are
part of `SlicedFormat` either way: a caller that can match the enum can already name them,
so nothing is exposed that was not reachable.

What has to be watched is a front end that wants a codec rather than a name — reading a
container's pixels, say. That is a dev-dependency if it is a test, and otherwise a sign the
pipeline is missing an operation. The signal to reopen is a front end needing a writer or a
reader directly.

## Alternatives considered

### Flavours of `core-pipeline`'s own, converted at the boundary

A `Revision` enum beside `SlicedFormat`, mapped onto each family's type where the writer is
called. Rejected: it is `printer-profiles::OutputFormat` again (ADR 0047), a third list of
the same thirteen containers to keep in step, for no gain over a re-export.

### Leave the dependencies and delete only the unused five

Honest and smaller. Rejected because it leaves the edge that is actually wrong: a front end
would still name `format-chitu` to say "revision 5", while the crate that writes revision 5
is reached only through `core-pipeline`.

### The option that won, and what it costs

`core-pipeline` now has names in its API that belong to crates under it, so a reader of
`core_pipeline::CtbVersion` has to follow the re-export to find where the type is defined
and documented. That is the price of the front ends not carrying eleven edges for four
names.
