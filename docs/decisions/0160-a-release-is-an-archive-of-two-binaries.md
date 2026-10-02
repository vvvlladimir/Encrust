# 0160. A release is an archive of two binaries, built in CI

- **Status:** Superseded by 0162
- **Date:** 2026-10-01

## Context

Going public needs something to download; building from source excludes most of the
audience, which runs Windows. The sibling project Stonqs ships installers through
`tauri-action`, but Encrust has no such host — it is a plain Cargo workspace with an
`eframe` binary and a CLI.

Two costs decide the shape. Code signing needs a paid Apple developer account and a
Windows certificate, and neither removes the warning until the certificate has reputation.
Installers need per-platform packaging work — `.msi`, `.dmg`, `.AppImage` — that has to be
maintained for three targets. Meanwhile four of five printer profiles have never printed
anything, so the thing most likely to harm a user right now is a release that looks more
finished than it is.

## Decision

A tag matching `v*` builds `encrust` and `slice` in release mode for four targets
— macOS aarch64 and x86_64, Linux x86_64 on the oldest supported runner, Windows x86_64 —
and uploads one archive per target, each holding the two binaries, `LICENSE` and
`README.md`.

The archives land on a **draft** release that a maintainer publishes by hand. `ci.yml`
owns correctness and the release workflow does not repeat it: a tag only ever sits on a
commit that already passed. A tag whose version disagrees with `[workspace.package]`
fails before anything is built.

No installers, no code signing, no update feed. `packaging/macos/bundle.sh` stays a local
convenience and is not wired into CI.

## Consequences

A release is one `git tag` and takes no platform knowledge to produce, so it can happen as
often as there is something worth shipping. The archive is auditable: two binaries and the
licence, nothing generated.

Users get an unsigned binary and an operating system warning, and the README has to explain
it. There is no in-app update path, so a fix reaches people only when they come back to the
releases page. Linux users outside the glibc floor of the oldest runner must build from
source.

Revisit when a printer other than the Saturn 4 Ultra has verified prints behind it — that
is the point at which signing and installers start paying for themselves rather than
polishing something nobody should rely on yet.

## Alternatives considered

### Installers from the start

Looks like a finished product and is what the competition ships. Rejected for now: three
packaging formats to maintain, and without signing the warnings remain anyway, so the
work buys appearance rather than trust.

### No binaries until the project is ready

Honest, and it is where the project has been. Rejected because the contribution the project
most needs — somebody with a printer trying it and reporting back — cannot come from people
who will not install a Rust toolchain first.

### Archives of unsigned binaries, and what it costs

Cheap, transparent and immediate. The cost is that the first impression on macOS and
Windows is a security warning, which loses some users before they ever open the window, and
that every update is manual.
