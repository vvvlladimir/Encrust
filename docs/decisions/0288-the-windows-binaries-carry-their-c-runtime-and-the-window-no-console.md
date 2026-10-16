# 0288. The Windows binaries carry their C runtime, and the window opens no console

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

A Rust binary for `x86_64-pc-windows-msvc` links the C runtime from `VCRUNTIME140.dll` unless
told otherwise, and is a console program unless its crate says it is not. The Store package
brings that DLL in through the `Microsoft.VCLibs.140.00.UWPDesktop` framework
([ADR 0286](0286-the-windows-build-also-ships-through-the-microsoft-store.md)); the installer
and the update archive bring nothing, so on a PC without the Visual C++ redistributable
`encrust-gui.exe` refuses to start. Where it does start, Windows opens a console beside the
window and fills it with the log.

## Decision

Every Windows MSVC build links the C runtime statically: `.cargo/config.toml` sets
`-C target-feature=+crt-static` for `cfg(all(windows, target_env = "msvc"))`. The Store
package no longer depends on the VCLibs framework, which replaces that sentence of ADR 0286.

`encrust-gui` is built for the `windows` subsystem. `encrust` stays a console program.

## Consequences

A downloaded `.exe` starts on a clean Windows install, and the window opens alone. Both
binaries grow by the size of the static runtime, a few hundred kilobytes. The window's log is
no longer visible on Windows, even when it is started from a terminal; `RUST_LOG` reaches
nothing there until the window writes its log to a file.

Reopen if a dependency ships a prebuilt library linked against the dynamic runtime: mixing the
two fails at link time or, worse, splits the heap between two runtimes.

## Alternatives considered

### Ship the redistributable with the installer

The installer would run Microsoft's `vc_redist.x64.exe`. Rejected: it needs administrator
rights the per-user installer does not ask for, and the update archive has no installer to
run it.

### Keep the console and hide it at start-up

`FreeConsole` on launch. Rejected: the console still flashes before it goes, and it needs a
Windows API dependency for what one attribute does.

### A static runtime, and what it costs

Larger binaries, a runtime fixed at build time rather than serviced by Windows Update, and no
log on screen for a Windows user to read when something goes wrong.
