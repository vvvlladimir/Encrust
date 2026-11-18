# 0461. Each release ships the Claude Desktop extension, one per installer's platform

- **Status:** Proposed
- **Date:** 2026-11-18

## Context

`encrust-mcp` reaches Claude Desktop as an `.mcpb` bundle (ADR 0455), which `cargo xtask mcpb`
packed on the machine it ran on, with the `zip` program. A user had to build it to have it.
Anthropic's directory no longer takes `.mcpb` listings, so nothing installs or updates one
for us: a user downloads the file and opens it, and Claude Desktop installs it. A release
builds six targets (ADR 0160, 0454) and attaches each one's archive and installers under
names without the version, so `releases/latest/download/<name>` always works. The window
knows its version and the repository it was built from, which is how it finds updates.

## Decision

- **The release builds `encrust-mcp`** beside the window and the command line on every
  target, and `cargo xtask mcpb --target <triple>` packs both into
  `encrust-<os>-<arch>.mcpb`, the names the archives use, attached with them. The bundle is
  written by `xtask` itself with the `zip` crate, the binaries marked runnable, so a Windows
  runner packs it as a Linux one does.
- **Signed where a certificate is set:** with the repository's `MCPB_CERT` and `MCPB_KEY`
  secrets, `mcpb sign` of `@anthropic-ai/mcpb` 2.1.2 signs each bundle; without them it ships
  unsigned, as it installs either way.
- **The window offers it:** File → *Use with Claude Desktop…* opens the bundle of the running
  version for its system and architecture in the browser, which downloads it. The browser
  build has no such row.
- **The site links** `releases/latest/download/encrust-<os>-<arch>.mcpb` beside the
  installers; the site is its own repository.

## Consequences

A user installs the extension with a download and a double click, with no window and no
build. A new release is a new download: nothing updates the extension in place, and the
extension does not tell when it is old. Each release builds one binary more per target. On
macOS the binaries in the bundle carry only the linker's ad hoc signature; whether Gatekeeper
lets Claude Desktop run them once it has unpacked a downloaded bundle is to be seen on the
first release, and notarising them is the remedy if not. Reopen when the directory takes
extensions again, or when the binaries need notarising.

## Alternatives considered

### The window writes Claude Desktop's configuration itself

No download at all — but the window would edit another program's file, whose place and shape
are Claude Desktop's to change, and the window holds no `encrust-mcp` to point it at.

### Packing with the `zip` program, as before

No new dependency for `xtask` — but a Windows runner has no `zip`, and 7-Zip keeps no Unix
mode, so a binary unpacked on macOS or Linux would not run.

### The option that won, and what it costs

One more binary built per target, a bundle per platform to keep named as the archives are,
and no update in place.
