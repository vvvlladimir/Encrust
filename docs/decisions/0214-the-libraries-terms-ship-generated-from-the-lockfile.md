# 0214. The libraries' terms ship with the binary, generated from the lockfile

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

Encrust is AGPL-3.0-only and `LICENSE` has travelled in every archive since the first
release. The libraries compiled into those binaries are not: `deny.toml` checks that each
one is under a licence AGPL can take, and the archives then carried none of their terms.
MIT, BSD-2-Clause, BSD-3-Clause and ISC each require their notice and their copyright line
to accompany a binary distribution, and Apache-2.0 asks for the NOTICE of a derived work
besides. With 436 MIT crates alone in the tree, those lines cannot be kept by hand, and a
list written once is wrong at the next `cargo update`.

The browser build is a distribution too: the module served from the site carries the same
libraries, and the AGPL's own § 13 is already answered there by the `Source` link in the
title bar (`panels::title_bar`).

## Decision

`cargo xtask licenses` writes `THIRD-PARTY-LICENSES.md` at the root of the workspace from
the lockfile, through `cargo-about` configured in `about.toml` and `about.hbs`, and
`--check` fails when the committed file is not what the lockfile would produce. The file is
committed, so no build step needs the tool installed: `release.yml` copies it into every
archive beside `LICENSE`, `cargo packager` takes it as a resource into each installer, and
`cargo xtask web` copies it into the browser build's directory.

`about.toml` accepts exactly the licences `deny.toml` allows, so a crate under anything else
fails the run instead of reaching a user with its terms unstated. The two lists change
together. The workspace's own crates are filtered out of the file: their terms are
`LICENSE`.

The staleness check runs in `audit.yml`, whose triggers are the lockfile and the manifests,
which is when the file can go out of date.

## Consequences

Every archive, installer and the browser build state the terms of what is in them, so
handing somebody a build no longer breaks the licences of the libraries it carries. A
dependency added without regenerating the file fails CI rather than shipping.

`cargo-about` is now a release-time tool to keep working, and its `cli` feature is not on by
default, so the install is `cargo install cargo-about --locked --features cli`. The file is
400 KB of licence text in the repository, and it churns on `cargo update`. The signal to
revisit is a licence whose text `cargo-about` cannot find in a published crate: that needs an
entry under `workarounds` in `about.toml`, which is deliberately empty so the gap is noticed.

## Alternatives considered

### A notices file written and maintained by hand

No tool to install and no generated 400 KB in the tree. It is also wrong the first time a
dependency moves, and nothing would say so — exactly the failure the licences punish.

### `cargo bundle-licenses`

Collects the same material. Its output is a data file meant to be rendered by something
else, and this project has nowhere to render it: the archives hand over a file a person
opens.

### A panel in the window listing the licences

The nicest for someone already running Encrust, and it reaches nobody holding the archive
or the `.deb`. It also puts 400 KB of text in the binary. Worth adding later beside the
file, not instead of it.

### The option that won, and what it costs

A generated file in the repository, 400 KB of it, that a reviewer has to skip past in every
diff that touches dependencies, plus one more tool a release depends on. In exchange the
terms shipped with a build always match the build, and CI says so.
