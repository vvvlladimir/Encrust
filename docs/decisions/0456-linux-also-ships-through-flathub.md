# 0456. Linux also ships through Flathub

- **Status:** Accepted
- **Date:** 2026-11-17

## Context

Linux ships as an `.AppImage` and a `.deb` ([ADR 0160](0160-a-release-is-an-installer-per-platform.md)).
Flathub is the store most distributions' software centres show, and it builds a submission
itself, from source, with no network: the Cargo crates come in as sources generated from
`Cargo.lock`, one of them a git revision (`boolmesh`). The app runs sandboxed, read-only under
`/app`, its files under `~/.var/app/<id>`, and reaches the user's files through portals.

Flathub needs from the app's own repository a MetaInfo file, a desktop entry and an icon, all
named by the app ID, and the MetaInfo must list the release being shipped. Its rules forbid
AI-generated or AI-assisted content in the Flathub manifest and AI-written pull requests,
commit messages and replies; AI-generated code in the app is allowed if disclosed.

## Decision

The app ID is `app.encrust.Encrust`, under the domain the site is served from, verified by a
file on encrust.app. This repository holds what the store reads:

- `assets/linux/app.encrust.Encrust.metainfo.xml`, listing every release; the release-please
  job that refreshes `Cargo.lock` adds the new version with the day's date to its pull request;
- `assets/linux/app.encrust.Encrust.desktop`, which opens a project given to it;
- `assets/linux/screenshots/`, read by the MetaInfo from `main`;
- the icon `assets/icon/encrust.svg` and the MIME type `assets/linux/encrust.xml`, installed
  under the ID's name by the manifest.

The window needs nothing beyond portals for files, the network for printers and the update
check, and the GPU. Under Flatpak it takes `FLATPAK_ID` as its Wayland app ID, and a copy
running from `/app` never replaces itself: it says Flathub updates it.

The manifest and its pull request to `flathub/flathub` are the maintainer's, written by hand,
and are not kept here.

## Consequences

A Linux user installs and updates Encrust from their software centre. Each release reaches
Flathub only when the maintainer bumps the manifest's tag and commit, and its crate sources
when `Cargo.lock` changed. A screenshot is changed by replacing its file on `main`.

The MetaInfo's release date is the day the release pull request last changed, not the day it
was merged, when the two differ. Reopen if Flathub accepts a release list it reads from
elsewhere, or if the window needs a static permission a portal does not cover.

## Alternatives considered

### `io.github.vvvlladimir.Encrust`

Verified by signing in to GitHub, with no file on the site. Rejected: it names a personal
account, and the ID cannot follow the repository to an organisation.

### The AppImage repackaged as `extra-data`

No build of ours to keep working on Flathub's builders. Rejected: Flathub builds open-source
software from source and keeps `extra-data` for what has no source.

### A Flathub copy that updates itself

Rejected: `/app` is read-only, and a second update path beside the store's is one the user
cannot see.

### A store we do not run, and what it costs

A manifest to keep in step with every release, by hand, outside this repository, and the
store's review between a fix and its Linux users.
