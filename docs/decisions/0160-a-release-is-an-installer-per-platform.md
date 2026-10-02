# 0160. A release is an installer per platform, built in CI

- **Status:** Superseded by 0162 (the trigger only)
- **Date:** 2026-10-02

## Context

Going public needs something to download, and most of the audience runs Windows or macOS and
will not install a Rust toolchain first. A loose binary is not an application to them: no icon,
no Applications folder, no Start menu entry, and on macOS a terminal window behind the program.

Code signing needs a paid Apple developer account and a Windows certificate, and neither removes
the warning until the certificate has reputation. Packaging, by contrast, costs nothing but a
build step. Encrust is a plain Cargo workspace with an `eframe` binary, so the packaging has to
come from a tool that works on any Cargo binary.

## Decision

The release workflow builds `encrust` and `slice` for four targets — macOS aarch64 and x86_64,
Linux x86_64 on the oldest supported runner, Windows x86_64 — and `cargo packager`, configured in
`crates/encrust-app/Cargo.toml`, wraps them into `Encrust.app` inside a `.dmg`, a per-user NSIS
`-setup.exe`, an `.AppImage` and a `.deb`. The icon is `assets/icon/encrust.icns` on macOS
(ADR 0135), `encrust.ico` embedded in the Windows executable by `build.rs`, and the PNG on Linux.

Each target also gets an archive of the two binaries, `LICENSE` and `README.md`: the payload of
the window's own update (ADR 0172), and a portable copy for whoever wants one.

Nothing is code-signed. A macOS user clears the quarantine flag once with
`xattr -dr com.apple.quarantine /Applications/Encrust.app`; a Windows user passes SmartScreen
with *More info → Run anyway*. Everything lands on a **draft** release that a maintainer publishes
by hand, and a tag whose version disagrees with `[workspace.package]` fails before anything is
built.

## Consequences

A user installs Encrust like any other application. The cost is a packaging tool installed on
every build leg, four installer formats whose layout we do not control, and a release page with
installers and archives side by side.

The window updates itself only where it can write its own binary: inside `Encrust.app`, in the
per-user Windows install, or from the archive. An `.AppImage` runs from a read-only mount and a
`.deb` belongs to the package manager, so those are sent to the release page. Linux users outside
the glibc floor of the oldest runner must build from source.

Revisit signing when the warnings demonstrably cost users, or when a printer other than the
Saturn 4 Ultra has verified prints behind it.

## Alternatives considered

### Archives of bare binaries only

Cheapest and fully transparent. Rejected: on macOS it opens a terminal, nothing has an icon, and
to most users it does not look like something to install.

### A packaging script per platform

`hdiutil`, NSIS and `appimagetool` driven by hand. Rejected: three scripts to maintain for what one
tool already does from a few lines of configuration.

### `cargo packager` installers, and what it costs

One more build-time tool whose output format we inherit, and unsigned installers that still meet
an operating-system warning on first start, so the README must explain the one extra step.
