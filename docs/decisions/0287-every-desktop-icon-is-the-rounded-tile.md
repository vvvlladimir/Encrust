# 0287. Every desktop icon is the rounded tile

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

[ADR 0135](0135-the-macos-icon-is-squircled-at-source.md) squircled the macOS icon and left
every other target the square `encrust.svg`. On a Windows taskbar and desktop that square is a
hard-edged black block beside rounded neighbours, and the Store package of
[ADR 0286](0286-the-windows-build-also-ships-through-the-microsoft-store.md) needs the same art
again in its own sizes.

## Decision

Windows and Linux use the macOS tile too, cropped tighter: `encrust-macos.svg` rendered with
its `viewBox` set to `72 72 880 880`, so the squircle fills 824 of 880 rather than 824 of
1024. `encrust-256.png`, every size in `encrust.ico` (16 to 256) and the logos in
`assets/msix/Images/` are rasterised from it with `rsvg-convert`, `encrust.svg` beside the
copy it reads.

## Consequences

The window, the executable, the installers, the Store package and the browser build's
installed icon all show one rounded shape. `lib.rs` still compiles a different file in on
macOS, which keeps the wider macOS margin. Two dozen generated PNGs now follow the drawing,
with nothing to say when one drifts; redrawing the logo means rerunning the renders.

## Alternatives considered

### The macOS grid unchanged

One fewer variant. Rejected: Windows and Linux draw an icon to the edge of its box, so the
macOS margin would leave the tile a fifth smaller than its neighbours.

### A crop of our own, and what it costs

A second margin to keep, defined only by a `viewBox` in this record rather than by a file of
its own.
