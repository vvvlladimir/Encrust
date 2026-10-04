# 0175. A sliced file leaves through a sink, not a path

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

By ADR 0045 every format writes into a seekable sink, and `core_pipeline::write_with`
already took `&mut dyn WriteSeek`. The public entry point above it did not:
`core_pipeline::write` took a `&Path`, called `File::create`, and used the path twice more
— for the name an `.sl1` gives its layer entries, and in `PipelineError::Write`.

A browser has no path. It writes into a buffer, or into an OPFS access handle inside a
worker, and both of those are a `WriteSeek` and nothing else. Leaving the path in the
signature would have meant a second entry point later and a changed public API in
`core-engine` on top of it.

## Decision

`Writing` carries a `name` — what the file is called, without its directory or extension —
instead of a path. `core_pipeline::write_to(&Writing, &mut dyn WriteSeek, &mut dyn Observer)`
is the entry point. `core_pipeline::write(&Writing, &Path, &mut dyn Observer)` stays as a
thin wrapper: create the file, buffer it, write into it, and remove it again when the run
fails or is cancelled.

`core_engine::Run` offers both the same way round — `write` into a sink, `write_file` for a
path — and `core_engine::project` does too, with `write_to`/`read_from` beside
`save`/`load`.

## Consequences

Nothing in the write path below the wrapper touches the filesystem, so the same call that
writes a `.goo` to disk writes one into a `Vec<u8>` in a test or a browser. The engine's
own round-trip tests write into a `Cursor` and read the bytes back, which is faster and
leaves no temporary files.

`PipelineError::Write` carries a name rather than a path, so an error from a sink that is
not a file still says which file it was. The caller that knows the path adds it with
`.context`, which both binaries already did.

A caller of `write_to` is responsible for throwing away what it wrote when the run comes
back `None` or an error. Only `write` does that for you. That is the one sharp edge, and
it is why the path wrapper stays rather than being pushed up into the front ends.

## Alternatives considered

### Keep the path and add a sink entry point beside it

Two public functions doing the same thing with the format dispatch duplicated between
them, or one delegating to the other with the path carried along unused. The second is
what we have, with the delegation the right way round.

### Keep `path` on `Writing` and ignore it in the sink call

Cheapest diff, and a struct field that is load-bearing for one of two callers and dead for
the other is exactly the kind of thing that is wrong in a year.

### The option that won, and what it costs

`write(&Writing { name, .. }, path, ..)` states the name twice over: once as `name`, once
inside `path`. `Run::write_file` derives one from the other so no front end has to, but a
direct caller of `core_pipeline::write` can pass a name that disagrees with its path, and
only an `.sl1`'s layer entries would show it.
