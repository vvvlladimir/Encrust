# 0455. The Windows installer also ships through winget

- **Status:** Accepted
- **Date:** 2026-11-17

## Context

Windows ships as an unsigned `-setup.exe` on the release page and as a Store package
([ADR 0286](0286-the-windows-build-also-ships-through-the-microsoft-store.md)). Windows 10 and
11 also come with winget, which installs from a community repository, `microsoft/winget-pkgs`:
a package there is three YAML manifests per version naming the installer's URL and SHA-256,
merged by pull request after a validation pipeline installs it in a sandbox. That repository
accepts an unsigned installer if it installs silently from a stable, version-specific URL.

The `-setup.exe` qualifies as it is: `cargo packager` builds it with NSIS, whose `/S` winget
passes by itself, per user (`installer-mode = "currentUser"`), under the uninstall key
`Encrust` with `DisplayVersion` the release's version, from
`releases/download/v<version>/Encrust-windows-<arch>-setup.exe`.

## Decision

The package is `Encrust.Encrust`, moniker `encrust`: installer type `nullsoft`, scope `user`,
product code `Encrust`, publisher `Encrust contributors` as the installer writes it, licence
`AGPL-3.0-only`. Its installers are the release's `-setup.exe` files, x64 and arm64.

Its first version, 0.8.0, is submitted by hand. From then on `.github/workflows/winget.yml`
runs when a release is published: Komac, pinned by version and SHA-256,
copies the last merged manifest, puts in the new version, URLs and hashes, and opens the pull
request from the maintainer's fork with a classic token of scope `public_repo`, the secret
`WINGET_TOKEN`. A failure costs only that pull request; the manual trigger resends a tag.

## Consequences

`winget install Encrust.Encrust` installs the same file as the download, so the window's own
update ([ADR 0172](0172-the-window-offers-a-signed-update-and-never-applies-one.md)) works
there unchanged, and winget's validation scan is the only check it adds; SmartScreen still
warns on a manual download.

A version reaches winget after its moderators merge it, hours to days after the release. The
self-update swaps the binaries but not the uninstall key's `DisplayVersion`, so after it
`winget upgrade` still lists Encrust until winget's own upgrade reruns the installer.

The token is a maintainer's, scoped to every public repository they own; it is replaced when it
expires, or the job fails at submission. Reopen if Komac stops reading manifests at the schema
version winget-pkgs moves to, or if the installer gets a signature or a machine-wide scope.

## Alternatives considered

### Winget's `msstore` source alone

The Store copy is already reachable as `winget install 9NB0ZMB3M8H8 --source msstore`. Kept,
not enough: it is the package the window cannot update, under an ID nobody guesses. With both,
a bare `winget install Encrust` matches two and asks for one, so the docs give the ID.

### An action wrapping Komac

Fewer lines in the workflow. Rejected: a third party's action holding the token, for a call
this short.

### Manifests kept in this repository

Reviewable here before they leave. Rejected: winget-pkgs is where they are validated and kept,
and Komac reads the last merged one, so a copy here would only drift.

### A package in a community repository, and what it costs

A release now raises a pull request in someone else's repository, and its moderators decide
when users get it; a maintainer's personal token sits in the repository's secrets.
