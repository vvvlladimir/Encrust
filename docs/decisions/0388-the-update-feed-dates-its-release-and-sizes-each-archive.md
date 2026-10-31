# 0388. Date the release and size each archive in the update feed

- **Status:** Accepted
- **Date:** 2026-10-31

## Context

The update sheet (ADR 0172, `docs/design/updates.md`) says what a release brings, but not
when it went out or how much it downloads, and the window cannot learn either before it
starts: the feed `latest.json` held a version, the notes, and per platform a URL and a
signature. A published release's time is not known while the job builds the draft.

## Decision

- `latest.json` gains `published`, an RFC 3339 UTC time, and, per platform, `size`, the
  archive's bytes. Both are optional to the reader: a feed written before still offers.
- The release job writes `published` as the time it runs, since a draft has none; the
  release-notes skill's publish command restamps it as the release goes out.
- The window reads the time with `jiff`, already a dependency (ADR 0387); a time it cannot
  read is left out of the sheet, never a reason to refuse the offer.

## Consequences

- The sheet and the Updates page state when a release went out and its download size.
- The feed has a field a script must restamp; a release published by hand keeps the draft's
  time, a few minutes to hours early.
- Reopen if the release host starts to state the published time in a place the window can
  read without a second request.

## Alternatives considered

### Ask the release host for the time and the size

Its API answers both, but it is a second request on every check, rate-limited without a
token, and the window would trust a second source besides the signed archive's feed.

### Leave the feed as it is and show the size once the download starts

The sheet could say nothing until bytes arrive; the user decides whether to download
before that, which is when the size matters.

### The option that won, and what it costs

Two fields the release job and the skill must keep writing, and a `published` that is only
as right as the restamp.
