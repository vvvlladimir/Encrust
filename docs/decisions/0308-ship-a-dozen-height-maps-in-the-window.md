# 0308. Ship a dozen height maps in the window

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

The Relief tool tiles any picture over a model (ADR 0225), but a user had to find a
seamless grey height map first, and most pictures on hand are neither seamless nor heights.
A texture worth pressing is a few hundred kilobytes, and whatever ships has to be ours to
ship under AGPL-3.0, in the desktop binary and the browser build alike.

## Decision

Twelve seamless height maps live in `assets/textures/` and are compiled into `encrust-app`
with `include_bytes!`: leather, matte grain, carbon weave, basket weave, honeycomb, hex
blocks, reptile, ripples, wood, bark, stone and rock. Each is a scanned material's
displacement map released as CC0 1.0, taken square at 1024 px, turned grey, stretched to the
full range from black to white and written as an 8-bit PNG of 512 px a side, about 1.7 MB
for all twelve. The Relief panel shows them as a grid of thumbnails; a click takes one as
the picture, exactly as a chosen file is taken, and like one it is not written into a
project (ADR 0191). The CLI keeps `--relief-image` and ships none of them.

## Consequences

Every build grows by the 1.7 MB, the browser download too. A texture is a file and a line
in `relief/library.rs`; a test decodes each and checks it is square. CC0 asks for no notice,
so nothing beside the files records where they came from. 512 px over a tile of a few
millimetres is finer than the lattice a relief is cut on; a texture that needs more is one
to choose from a file.

## Alternatives considered

### Patterns drawn by code

No files at all and any resolution, but only geometric patterns come out well; leather,
wood, bark and stone are what people texture prints with.

### Fetching textures on demand

A small binary, but the window would need the network to show its own tool, and the browser
build a server to fetch from.

### The option that won, and what it costs

Twelve files in the repository and in every binary, and a set chosen by eye that only an
edit to the list changes.
