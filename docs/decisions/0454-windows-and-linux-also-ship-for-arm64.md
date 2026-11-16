# 0454. Windows and Linux also ship for ARM64, and the Store takes a bundle

- **Status:** Accepted
- **Date:** 2026-11-17

## Context

A release builds Windows and Linux for x86_64 only ([ADR 0160](0160-a-release-is-an-installer-per-platform.md)).
On an ARM64 Windows laptop that build runs under emulation, slower on exactly the rayon-bound
slicing that matters; on an ARM64 Linux board it does not run at all.

Nothing in the workspace stands in the way: no architecture-specific code, no C or C++ library
but `ring`, which compiles its C with clang on ARM64 Windows. GitHub's `windows-11-arm` and
`ubuntu-22.04-arm` runners are free for a public repository; the first carries LLVM on its path,
the second the oldest glibc an ARM runner offers. `cargo packager` 0.11.8 maps `aarch64` to an
`arm64` NSIS installer and `.deb`, and fetches the `aarch64` linuxdeploy for the AppImage.

The Store takes one submission per release. A package carries one architecture; several reach
the Store as an `.msixbundle`, and once an app has shipped a bundle Partner Center refuses a
plain package for it.

## Decision

`release.yml` builds `aarch64-pc-windows-msvc` on `windows-11-arm` and
`aarch64-unknown-linux-gnu` on `ubuntu-22.04-arm` with the same steps as x86_64. Their files
carry the architecture as macOS's do: `encrust-windows-aarch64.zip`,
`Encrust-windows-aarch64-setup.exe`, `encrust-linux-aarch64.tar.gz`,
`Encrust-linux-aarch64.AppImage`, `encrust_arm64.deb`. The feed keys them `windows-aarch64` and
`linux-aarch64`, which the window maps its own target to.

Both Windows legs hand their binaries to a `store` job, which packs an MSIX per architecture
from `assets/msix/`, the manifest's `@ARCH@` filled with `x64` or `arm64`, bundles them with
`makeappx bundle` at the package version, and attaches `Encrust-windows-store.msixupload`. A
missing architecture fails the job rather than bundling one.

The nightly CI runs clippy and the tests on `windows-11-arm` and `ubuntu-24.04-arm`.

## Consequences

ARM64 Windows and Linux users get a native build, its installer and the window's own update.
A release costs two more build legs and a job, and the nightly two more runs.

A CI runner has no GPU, so whether the window draws on an Adreno or a Raspberry Pi is checked
on the device, not in CI. The Store is bundled from now on, for good.

Reopen if an ARM64 leg breaks more releases than its users justify, or a crate starts
needing a C toolchain the ARM runners lack.

## Alternatives considered

### Cross-compiling ARM64 on the x86_64 runners

No extra runner type. Rejected: Linux would need an ARM64 sysroot with GTK for the link and an
emulated linuxdeploy for the AppImage, and neither platform's tests would run on ARM64.

### Two Store packages without a bundle

Keeps a package as it was. Rejected: the publishing CLI sends one file per submission, and a
`.msixupload` holding several packages is what a bundle is for.

### The option that won, and what it costs

A Store bundle cannot be undone, and a release whose ARM64 Windows leg fails reaches no Store
user, since the `store` job will not bundle x64 alone.
