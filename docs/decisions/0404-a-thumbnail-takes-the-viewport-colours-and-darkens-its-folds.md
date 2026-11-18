# 0404. Draw the thumbnail in the viewport's colours, darken its folds, dither it into 16 bits

- **Status:** Proposed
- **Date:** 2026-11-05

## Context

After ADR 0403 the thumbnail is smooth and outlined, but three things still read poorly on a
printer's screen. Supports are the model's colour, so a supported part is one blue mass. A
figure's folds and crevices are as bright as its open surfaces. And most formats store the
picture in 16-bit words, five or six bits a channel, which turns every smooth gradient into
visible bands. The picture's blue was also chosen for the thumbnail alone, unlike the resin
and supports the user sees in the window.

## Decision

- `core-thumbnail` holds `RESIN` (`#86ABBB`) and `SUPPORT` (`#C9C2B6`); the window's dark
  viewport takes its colours from them. A `Part` carries a `Look`, and `core_engine::parts`
  marks supports as such.
- Each covered pixel's diffuse light is scaled by an occlusion taken from the depth buffer:
  opposite samples at two radii, compared with the pixel's depth. A line of ADR 0403 is
  drawn only where the jump also passes 2.5% of the parts' extent, and supports take
  neither line nor occlusion. Numbers: `docs/design/thumbnail.md`.
- Every writer that packs RGB565 or RGB15 calls `Thumbnail::dithered` first, a 4 x 4 ordered
  dither of one step of the kept bits.

## Consequences

The file's picture and the viewport show the same resin and supports, and changing one
changes both. Folds read as depth. Gradients survive the 16-bit records; a run-length
record (`.ctb`, `.cxdlpv4`) grows over the model's area, where runs were short already, and
not over the black background, which a dither leaves black.

A render of a 978k-triangle sphere, cut and dithered, takes 148 ms on an Apple M1, from
100 ms; the occlusion reads 16 depths a pixel.

The light palette's resin colour is still its own: a thumbnail always sits on a dark
screen. Reopen if a printer's screen turns out light.

## Alternatives considered

### Pass the colours from the window into the job

The picture would follow the palette in use. It lost because the CLI has no palette and must
write the same file as the window (ADR 0048).

### Error diffusion instead of an ordered dither

Floyd-Steinberg hides the bands better at the same bit depth. It lost on runs: its noise
reaches the background next to the model and breaks the runs `.ctb` stores, and its
pattern changes with every pixel the model moves.

### The option that won, and what it costs

A screen-space occlusion only sees what the camera sees, so a cavity hidden behind a lip
does not darken what is in front of it, and its radii are tuned by eye. The theme now
depends on a core constant for two of its colours.
