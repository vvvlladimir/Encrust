# 0385. Keep a plate's picture as shade and coverage, tinted when shown

- **Status:** Accepted
- **Date:** 2026-10-31

## Context

The start page pictures a recent file by a PNG drawn once, in the palette in force when the
file was opened or saved (ADR 0234), and a project carries the same PNG as `preview.png`
(ADR 0261). Since the window has a light palette (ADR 0383), a file saved in the dark showed a
black tile on the light page: a texture cached across frames kept the old theme.

## Decision

A picture is rendered white on black and kept as a grey and alpha PNG: grey the mean shade of
the covered samples, alpha the share covered. The start page tints it with this frame's tile
background and model colour on loading, and caches its texture per theme. `preview.png` in a
project is the same picture. An RGB picture, kept before this, is shown as it was drawn.

## Consequences

Every picture, a project's included, follows the theme without being drawn again. A
`preview.png` seen outside the window is a grey model on transparency. Rendering stays
two-sided Lambert from one colour; a picture wanting two colours, a support tint say, would
need a channel of its own and reopens this.

## Alternatives considered

### Draw and keep one picture per theme

Twice the drawing and the files, and a project's own picture would still be in one theme.

### The option that won, and what it costs

A picture is no longer an image to show as it is: whatever shows it must tint it, and the
pictures kept before this stay in their old colours until the file is opened again.
