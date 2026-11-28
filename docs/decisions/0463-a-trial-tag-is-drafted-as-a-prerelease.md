# 0463. Draft a trial tag as a prerelease

- **Status:** Accepted
- **Date:** 2026-11-27

## Context

`release.yml`'s manual trigger builds a tag that exists and refuses one whose version is not
`Cargo.toml`'s (ADR 0162). A build from a branch, such as the chat-app extension before it is
merged (ADR 0455), could then only be hung on a published release, where every visitor sees it.
`releases/latest`, the update feed and the site's pin never name a prerelease.

## Decision

A trial tag is `Cargo.toml`'s version with a suffix, `v0.9.0-try1`. Run by hand on a trial tag
that does not exist, `release.yml` makes it on the commit the run starts from, accepts it
against `Cargo.toml` by the part before the suffix, and drafts its release as a prerelease. The
Store package is skipped for it, as an MSIX version carries no suffix. A release's own tag stays
release-please's to make: a missing tag without a suffix is refused.

## Consequences

A branch is tried with every build of a release, nothing public until a maintainer publishes,
and even then never the latest. Binaries built so still say the version without its suffix, so
a window's own update and its chat-extension download look for that version, not the trial's.
Trial tags pile up until deleted by hand.

## Alternatives considered

### Pack from another ref onto an existing tag

`mcpb_ref`, as `web_ref` does for the site: it hung a branch's bundles on a published release.

### Upload the bundles as the run's artifacts

Nothing public — but only the bundles, and a download unlike the one a user makes.

### The option that won, and what it costs

A tag and a draft per trial, which someone deletes; and the trial's files carry the plain
version, so they test the build, not the window's download of it.
