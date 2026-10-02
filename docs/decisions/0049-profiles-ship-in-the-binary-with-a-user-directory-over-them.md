# 0049. Ship the profile catalogue inside the binary, with a user directory over it

- **Status:** Accepted
- **Date:** 2026-09-20

## Context

Until step 8b a printer was a path: `--profile assets/profiles/elegoo-mars-4-ultra.toml`
on the command line, a file dialog in the window. That works for someone who cloned the
repository and nobody else. A user who downloads a binary has no `assets/` directory, so
the first thing the slicer asks them for is a file they do not have.

The catalogue also has to be editable. Every number in it is transcribed from a vendor
specification rather than measured, and the person holding the machine knows better than
we do. Their correction must survive an upgrade of the binary, and it must be reachable
without them learning where the executable lives.

## Decision

Every `.toml` under `assets/profiles/printers` and `assets/profiles/resins` is embedded
into `printer-profiles` at build time. A build script walks both directories and emits an
`include_str!` table, so adding a profile to the repository is adding a file and nothing
else.

A profile is addressed by an id, which is its file stem: `elegoo-mars-4-ultra`, not a
path. `Catalogue::load` reads the embedded table and then lays the user's directory over
it by id, so a file the user puts at `<config>/Encrust/profiles/printers/<id>.toml`
replaces the shipped machine of that id and a new id adds a machine. The directory is the
platform configuration directory through the `directories` crate, or whatever
`ENCRUST_PROFILE_DIR` names, which is what the tests use so that they never read the
real one. A missing directory is not an error; a directory that cannot be read, or a file
in it that does not parse, is reported.

A path still wins over an id everywhere: `--profile` beats `--printer`, and the window's
"Open a profile file..." beats the menu above it. Tuning a profile by hand never requires
putting it in the catalogue first.

## Consequences

A fresh install slices for five machines with no file editing, and `slice
--list-profiles` says which. The binary grows by the size of the catalogue, which is a
few kilobytes of TOML and parsed once at startup rather than at every use.

Profile ids are now a compatibility surface: renaming a file in `assets/profiles` silently
orphans the user override that carried the same name. Ids are therefore treated as stable,
and a machine that gets renamed keeps its id.

The build script makes `cargo build` depend on a directory outside the crate. `cargo`
reruns it when either directory changes, but a profile added while a build is in flight is
missed until the next one. If the catalogue ever grows past a few hundred files, embedding
all of it becomes the wrong trade and this gets revisited.

## Alternatives considered

### Read the catalogue from a directory next to the executable

No build script, and a user can drop a file in without finding the configuration
directory. It loses the guarantee that matters most: an installed binary that has been
moved, or one run out of a `target/` directory, finds nothing and offers an empty picker.
A slicer that cannot name a single printer on first run is the problem this step exists to
fix.

### A hand-written `include_str!` list

Explicit, greppable, no build script at all. Rejected because the list is the kind of
thing that is forgotten: a profile added without its line is invisible, and nothing fails.

### The option that won, and what it costs

Embedding plus an override directory means there are two places a printer can come from,
and a user who edits `assets/profiles` in a clone will not see their change until they
rebuild. It also means the shipped catalogue can only be corrected by a release, so a
wrong number reaches every user until one goes out. `ENCRUST_PROFILE_DIR` and the
`[yours]` mark in the picker exist to make which file won visible rather than mysterious.
