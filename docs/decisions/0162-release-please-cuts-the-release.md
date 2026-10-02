# 0162. release-please cuts the release, and the build workflow is called by it

- **Status:** Accepted
- **Date:** 2026-10-01
- **Supersedes:** 0160

## Context

[ADR-0160](0160-a-release-is-an-archive-of-two-binaries.md) settled what a release contains — two
binaries, the licence and the README, four targets, a draft a maintainer publishes — and said a tag
matching `v*` triggers the build. Everything in it about the contents still holds; the trigger does
not survive contact with automation.

Two facts forced the revision. Conventional Commits are already the rule here, so the version number
and the changelog are derivable from the history rather than things to write by hand, and a release
cut by hand leaves the version in `Cargo.toml` and the tag free to drift apart. And a tag pushed by a
workflow using `GITHUB_TOKEN` raises no event, so `on: push: tags` would never fire for any tag a bot
creates — the moment tagging is automated, the trigger in 0160 is dead code.

## Decision

`release-please` owns the version and the changelog. It keeps one open pull request carrying the next
version and the entries earned since the last release; merging it writes `CHANGELOG.md`, bumps
`[workspace.package] version`, tags, and opens the draft release. A second job in the same workflow
refreshes `Cargo.lock` on that pull request, because release-please edits the manifest and knows
nothing about the lock, and the release build runs `--locked`.

`release.yml` becomes `workflow_call`, invoked by `release-please.yml` after the tag exists, plus a
`workflow_dispatch` for rebuilding a tag whose run failed. It no longer listens for a tag at all. It
still refuses a tag whose version disagrees with `Cargo.toml`, and it still only uploads to a draft:
publishing stays a human act.

The archive contents, the four targets, the absence of installers and of code signing are unchanged
from 0160.

## Consequences

A release is a merged pull request. The changelog is whatever the commit subjects said, which makes
the commit message rule in `CONTRIBUTING.md` load-bearing rather than decorative, and makes a sloppy
subject a user-visible defect.

Three new files have to stay consistent with each other: `release-please-config.json`, the manifest
holding the current version, and `Cargo.toml`. The `bootstrap-sha` pins where changelog history
begins — the commit that published the project — so nothing from before it is mined for entries.

The release path is now two workflows deep, which is harder to debug than a tag push: a failure in
`release-please.yml` leaves a tag with no archives, and the fix is the manual trigger rather than
re-pushing anything.

Revisit if release-please ever stops supporting a plain Cargo workspace, or if the lockfile job turns
out to race with the release pull request's own commits.

## Alternatives considered

### Keep tagging by hand, as 0160 had it

Fewer moving parts, and it worked. Rejected because the version then lives in two places that drift,
and the changelog becomes a file somebody forgets — with the commit convention already enforced, both
come free.

### Let release-please push the tag and keep `on: push: tags`

The smallest edit to 0160. Rejected because it does not work: a tag pushed with `GITHUB_TOKEN` fires
no event, so the build would silently never run.

### A personal access token so the bot's tag does raise an event

Would preserve 0160's trigger exactly. Rejected: a long-lived token with write access, stored as a
secret, to avoid one `workflow_call` line.

### release-please plus `workflow_call`, and what it costs

The chosen shape. Its honest cost is the indirection — the thing that builds a release is no longer
reachable from the thing that made the tag by reading one `on:` block, and a contributor looking for
"what happens when we release" has to read two workflows and three JSON files.
