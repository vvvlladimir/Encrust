# 0135. The macOS icon is squircled at source, not only in the bundle

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

The compiled-in icon was chosen on the assumption that macOS ignores
`ViewportBuilder::with_icon` and reads the Dock icon out of the `.app` bundle's `.icns`. That is true of winit but not of eframe: on macOS
`eframe::native::app_icon` turns the icon bytes into an `NSImage` and calls
`setApplicationIconImage`, which overrides the bundle's `.icns` for the life of the
process. The compiled-in art is the plain square, so the Dock showed a square tile —
under `cargo run` and inside the bundle alike. macOS never masks an icon itself; the
squircle and its transparent margin have to be in the pixels, 824 of a 1024 canvas.

## Decision

`assets/icon/encrust-macos.svg` is the macOS grid: a 1024 canvas that places
`encrust.svg` at 824 wide and clips it to an n=5 superellipse. `encrust-macos-512.png`
is rasterised from it and compiled into the binary under `cfg(target_os = "macos")`;
every other target keeps the square `encrust-256.png`. `packaging/macos/bundle.sh`
rasterises the same SVG into the iconset, so the Dock, the bundle and the Finder all
carry one shape. The polygon in the clip path is generated, not drawn by hand.

## Consequences

The Dock is right however the app is started, which is what a developer running
`cargo run` actually sees. `bundle.sh` loses its inline Python and its dependency on a
`python3` on the path. One more checked-in PNG to regenerate when the drawing changes —
two `rsvg-convert` calls now, both listed beside the assets. If eframe ever stops
setting the application icon on macOS, the `cfg` can go and the bundle alone will do.

## Alternatives considered

### Skip `with_icon` on macOS

Two lines, and the bundle's `.icns` would be honoured. But a bare `cargo run` would
fall back to the generic executable icon, which is the case that prompted this.

### Mask at startup instead of at rest

No second PNG, but superellipse maths and an alpha pass in `encrust-app` to compute at
every launch what one `rsvg-convert` computes once.

### The option that won, and what it costs

A third generated file under `assets/icon`, and nothing enforces that any of them came
from the current `encrust.svg` — the same drift already accepted when the first two were
generated, now over three derivatives rather than two.
