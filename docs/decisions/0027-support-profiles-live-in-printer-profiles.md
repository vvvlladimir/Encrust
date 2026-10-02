# 0027. Keep the support profile in printer-profiles

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

A support has a shape: how wide the tip is, how far it bites into the model, how thick the
pillar is, how big the foot is. These are numbers a user tunes for their resin and their
parts, and carries between machines. Every MSLA slicer ships them as named presets, and they
all expose roughly the same measurements under roughly the same names.

The project already loads two kinds of TOML profile. `printer-profiles` owns
`PrinterProfile` and `MaterialProfile`, their validation, their error type and their
loader, and is a leaf of the dependency graph with no workspace dependencies. Shipped
examples live in `assets/profiles/`.

The alternative home is `core-supports`, which owns the algorithms that read these
numbers, but which depends on `core-geometry` and would have to grow a second serde and
`toml` surface to do it.

## Decision

`SupportProfile` lives in `printer-profiles` alongside the other two, with the same
`from_toml_str`/`load` pair and the same `ProfileError`. `ProfileError` grows `TooFew` and
`OutOfOrder` for the constraints a support shape has and the other two do not.

The dependency graph gains one edge:

```
core-supports ──> core-geometry, printer-profiles
```

Three presets, `light()`, `medium()` and `heavy()`, are compiled in, and
`assets/profiles/supports/{light,medium,heavy}.toml` carry the same numbers. An
integration test asserts the files and the presets have not drifted apart.

## Consequences

There is one crate that parses profile TOML, one error type to match on, and one place a
user looks for a profile file. `core-supports` reads a `SupportProfile` directly, with no
conversion type in between and no duplicated field list. A support profile can be loaded
from a file the moment a picker is added for it, without new code.

`printer-profiles` is no longer only about the printer and the resin. The name is now a
little wider than what it holds, and a fourth kind of profile will widen it again.

Reopen this if profile loading grows beyond TOML, or if `core-supports` ever needs a field
that only makes sense as geometry rather than as a number.

## Alternatives considered

### Put `SupportProfile` in core-supports

It sits next to the code that uses it, and keeps `printer-profiles` about printers. It was
rejected because it puts a second `serde` and `toml` dependency, a second error enum and a
second loader into the workspace for the same job, and because a user would then have two
directories to look in for the same kind of file.

### Keep the numbers in core-supports and the file format in printer-profiles

Data in one crate, parsing in the other, joined by a `From` in the binaries. Rejected
because neither crate can write that `From`: the binaries would each carry a field-by-field
conversion that has to be updated whenever the profile grows a field, and forgetting one is
silent.

### Put it in printer-profiles, which is what we do

The cost is the crate's name and scope. It is now the profile crate rather than the printer
crate, and rule 5 of `AGENTS.md` will eventually push for a rename or a
split if a fourth profile kind arrives. The new dependency edge is also real: `core-supports`
can no longer be used without `printer-profiles`, so a caller that wants to generate columns
from numbers it computed itself has to build a `SupportProfile` to say so.
