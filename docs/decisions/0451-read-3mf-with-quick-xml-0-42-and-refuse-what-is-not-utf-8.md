# 0451. Read 3MF with `quick-xml` 0.42, and refuse a model part that is not UTF-8

- **Status:** Proposed
- **Date:** 2026-11-16

## Context

ADR 0395 reads 3MF through `quick-xml` 0.41. Release 0.42 hands names, attribute values and
text out as `&str` instead of bytes, and its reader now checks that its input is UTF-8 while
it builds each event, failing with `Error::Encoding` where 0.41 passed the bytes through.
The 3MF core specification requires a model part to be UTF-8. Until now a part that broke
this only where we never look, such as an object's `name`, loaded anyway.

## Decision

`core-mesh-io` reads 3MF with `quick-xml` 0.42. `three_mf/parse.rs` and `three_mf/package.rs`
match names and values as `&str`, and no longer decode or lossily convert bytes themselves.
A model part that is not UTF-8 anywhere is `MeshIoError::Malformed`, the error 0.42 gives it.

## Consequences

The parser gets shorter: `number`, `finite` and `matrix` lose their own UTF-8 step, and an
error message quotes the value as it is. A file that broke UTF-8 only in a part we skip, which
0.41 opened, is now refused, and the test `a_model_part_that_is_not_utf8_is_malformed` holds
that. The signal to revisit is a user's file from a real exporter that is refused this way.

## Alternatives considered

### Stay on 0.41

It costs nothing now. But every later `quick-xml` fix lands only in 0.42 and on, and the port is
two files.

### Transcode through 0.42's `DecodingReader`

That transcodes a source that declares another encoding. It cannot repair bytes that claim to be
UTF-8 and are not, which is the case seen here, and it pulls in the `encoding` feature for a
format whose specification allows only UTF-8.

### Refusing, as chosen

The specification backs the refusal, but a file that a lenient reader would have opened is lost
to the user until its author fixes the exporter.
