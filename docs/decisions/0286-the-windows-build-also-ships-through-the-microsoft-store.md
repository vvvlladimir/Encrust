# 0286. The Windows build also ships through the Microsoft Store

- **Status:** Accepted; the manual upload is replaced by `.github/workflows/store.yml`, which sends the package when the release is published
- **Date:** 2026-10-15

## Context

The Windows installer is not code-signed ([ADR 0160](0160-a-release-is-an-installer-per-platform.md)),
so SmartScreen warns on every download and Smart App Control may refuse it outright. A
certificate a CA would issue to a person costs money every year and lives on a hardware token
a CI runner cannot reach; the free signing services for open-source projects either require
an established reputation or exclude individuals in the EU.

The Microsoft Store registers an individual publisher for free and signs an MSIX package
itself, so a copy installed from it raises no warning. It installs that package read-only
under `WindowsApps`, where the window's own update ([ADR 0172](0172-the-window-offers-a-signed-update-and-never-applies-one.md))
cannot replace a binary, and it updates the package itself.

## Decision

The Windows leg of `release.yml` also builds an MSIX from the same two binaries: the manifest
and its logos are `assets/msix/`, the identity is the one Partner Center reserved, the version
is the release's with a fourth number of `0`, which the Store reserves. `makepri` and
`makeappx` from the runner's Windows SDK build it unsigned, and it is attached to the draft as
`Encrust-windows-x86_64-store.msixupload`, which a maintainer uploads to Partner Center.

`encrust.exe` is reachable from a terminal through an app execution alias. The package
depends on the `Microsoft.VCLibs.140.00.UWPDesktop` framework for the C runtime.

A window that runs from under a `WindowsApps` directory never reads the update feed and never
replaces itself; its Updates page names the Store and links to the app's page there.

## Consequences

A Windows user who installs from the Store sees no warning and is updated by the Store, which
also means the Store copy makes no request to GitHub. The direct download stays as it is,
unsigned, with the window's own update.

A release reaches Store users only after a maintainer uploads it and the Store certifies it,
hours to days after the GitHub release. The upload is manual until the Store's command-line
publisher, which needs an Entra application with access to Partner Center, is wired into CI.
A Store copy keeps its settings apart from an installed copy's: Windows redirects a packaged
app's writes to `%APPDATA%` into the package's own storage.

Reopen if the direct download gets a code signature: the Store also accepts a signed `.exe`
installer by URL, which would keep one package and one update path.

## Alternatives considered

### A certificate of our own

Signs every download, not only the Store's. Rejected for now: a yearly cost while the project
earns nothing, and a hardware-held key the release workflow cannot use unattended.

### Detecting the package through `GetCurrentPackageFullName`

The documented test. Rejected: it needs a Windows API dependency for an answer the path
already gives, since Windows installs every package under `WindowsApps`.

### A second package in the Store, and what it costs

Two Windows builds to describe and two ways a Windows user is updated, a manual upload per
release, and a store's certification between a fix and its users.
