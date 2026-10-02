# 0172. The window offers a signed update, and never applies one by itself

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

A release ([ADR 0162](0162-release-please-cuts-the-release.md)) is a page somebody has to
remember to visit, so a fix reaches nobody who does not come back. Encrust is a plain
`eframe` binary with no application framework underneath, so there is no updater to switch
on: the check, the download, the signature and the swap have to be built here.

Three commitments bind it. The README promises no network call the user did not ask for.
The network crates carry no TLS stack, and GitHub serves nothing without one. An archive
is unsigned by any platform authority ([ADR 0160](0160-a-release-is-an-archive-of-two-binaries.md)),
so the download needs a signature of the project's own, or whoever controls the feed or the
connection controls every user's binary.

## Decision

The update is **offered, never applied**. Settings › Updates has a switch, off by default,
that lets the window read the feed once a day; "Check now" reads it on request. A newer
version shows a badge in the title strip and its release notes on the page. Only "Install
and restart" downloads anything.

`release.yml` gains a `feed` job: it signs every archive with minisign, puts
`encrust <version> <archive>` in the signature's trusted comment, verifies the result against
`assets/update-key.pub`, and uploads the `.minisig` files and `latest.json` to the draft.
The window reads `releases/latest/download/latest.json`, which resolves only once a draft is
published.

The window refuses an archive URL outside its own repository's release of that version,
verifies with `minisign-verify` against the key compiled in from `assets/update-key.pub`, and
checks the trusted comment, so a signature cannot be moved to another file or replayed for
an older version. It then writes the new `encrust`, and `slice` when one stands beside it,
over the old ones by rename; on Windows the running binary is moved aside first and removed
on the next start. A development build is never replaced.

TLS is `ureq`'s `rustls` feature with the `webpki-roots` certificates, turned on in
`encrust-app` alone. `tar` and `flate2` read the `.tar.gz` archives; `zip` already reads the
Windows one. How it works in detail: `docs/design/updates.md`.

## Consequences

A user two versions behind sees the badge the day after the release is published and is
current in two clicks. Publishing the draft stays the one act that reaches users.

The private key is a repository secret, `UPDATE_SIGNING_KEY`, and its loss means a release
nobody can verify: the key must be backed up outside GitHub. Rotating it ships a new public
key, which only builds from then on trust, so a rotation strands every older install on the
manual path. The window now carries a TLS stack and a certificate list that ages with the
build. `deny.toml` admits `CDLA-Permissive-2.0`, the licence of those certificates.

The swap works for the archive layout `release.yml` produces. An install a package manager
owns, or a directory the user cannot write, fails with the path named, and the release page
is the way then. Revisit when installers arrive: each brings its own update mechanism.

## Alternatives considered

### An update check on by default

Reaches more people. Rejected: it breaks the promise that the window makes no call the
user did not ask for, which is worth more than the reach.

### The GitHub releases API instead of a feed file

No extra file to build. Rejected: it is rate-limited per address and its answer carries no
signature, while the feed puts each signature beside the URL it covers.

### Checksums in the feed instead of a signature

Simpler to produce. Rejected: whoever can change the feed can change the checksum, so it
only catches a broken download, not a hostile one.

### The OS certificate store instead of bundled roots

Follows a corporate proxy's certificate. Rejected for now: a larger dependency for a case
nobody has reported, and the signature, not TLS, is what makes the download trustworthy.

### minisign and an in-place swap, and what it costs

A signing key the project has to keep safe for as long as any build trusts it, a TLS stack
in a binary that until now spoke only plain HTTP on the local network, and a swap written
here rather than an installer's, which is one more piece of platform-specific code to keep
right on Windows.
