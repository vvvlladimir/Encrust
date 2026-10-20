# 0303. Do exact mesh booleans in `core-csg`, over `boolmesh`

- **Status:** Accepted; a result's triangle list varying from run to run is superseded by 0315
- **Date:** 2026-10-18

## Context

The lattice of a hollow needs cells and holes cut exactly from a cavity (ADR 0302), and the
plan's next steps need the same operation between models (union, subtract, intersect),
under raised or sunk text, and with primitives. The workspace had booleans only by
sampling a distance field, which rounds every edge to the voxel. An exact boolean over
triangle meshes is a hard algorithm to make robust, and the browser build (ADR 0177) rules
out anything that needs a C++ toolchain for `wasm32`.

Candidates, measured on a sphere minus a hundred prisms: `boolmesh` 0.2, a pure Rust port of
Manifold, MPL-2.0 — 1.0 s at a million faces, 0.2 s at 200 k, and it builds for `wasm32`;
`manifold3d`, bindings to the C++ original, faster and older but a C++ build on every
target; `csgrs`, BSP trees, which slow down sharply past tens of thousands of faces.

## Decision

A new crate, `core-csg`, on `core-geometry` and `boolmesh` alone. It exposes `Solid` — a
closed mesh read once, or the empty solid — with `union`, `difference` and `intersection`
answering new solids, `mesh` back to a `Mesh`, and `CsgError` with `NotSolid` and `Failed`.
No caller names `boolmesh`. `boolmesh` is built with `rayon` and in `f64`.

The closed meshes callers hand it are built in `core-geometry`: `cuboid`, `oriented_box`,
`prism`, `cylinder` and `segments_for`.

## Consequences

The engine can be swapped for `manifold3d` or a later `boolmesh` inside one crate. Model
booleans, text and primitives (plan items MESH-1 to MESH-3) take `Solid` as it is.

`boolmesh` brings a second `glam`, 0.32 beside the workspace's 0.33, until it moves. It walks
hash maps, so the same boolean triangulates flat polygons differently from run to run: a
result is reproducible in its surface, not in its triangle list, and tests compare volumes
and counts. Using `BTreeMap` in its `boolean45` would fix that; it is worth sending upstream.

Its input must be a solid: open or self-crossing meshes are refused. It panics on an edge
more than two faces use, so `Solid::new` splits such seams first, drawing the copies of a
split vertex `1e-4` mm apart, and turns any panic of the engine into an error — on the
desktop only: `wasm32` aborts on a panic, so in the browser the seam splitting is the whole
guard and a panic the engine still raises ends the page. A caller that
has to answer anyway keeps a fallback — the lattice falls back on the field (ADR 0302), and
model booleans will need to weld and check first.

Reopen if `boolmesh` stalls, if a real model it refuses turns out common, or if its speed
holds a feature back; `manifold3d` is the next candidate.

## Alternatives considered

### `manifold3d`

The reference implementation, and the fastest. Its C++ core would have to be built for
every desktop target and, in practice, could not be built for the browser at all.

### Booleans through the distance field

Already in `core-volume` and robust on any input, but it rounds every edge to the voxel and
costs the volume rather than the surface; it stays the fallback, not the tool.

### The option that won, and what it costs

A young crate with one main author, a second `glam`, and results whose triangle lists are
not reproducible. Wrapping it in `core-csg` is what keeps that risk in one place.
