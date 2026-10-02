# 0121. A map may cover part of a mesh

- **Status:** Accepted
- **Date:** 2026-09-28

## Context

ADR 0115 decided that a file mapping only some of its objects yields no map at all, on the
grounds that a hole cannot be told from a coordinate at the texture's origin. Real models
are the other way round: a statue downloaded as one OBJ is eighty parts, of which the flame
and the preview material carry no `vt` at all, and refusing the whole file for that leaves
nothing to press a relief into.

Two more things in the way are the files themselves, not the rule. Exporters write the
absolute path the image had on the machine that made them — `E:\Tanks\...\hull.dds` — and
`tobj` splits the `mtllib` line on a space alone, so a file separating the keyword from the
name with a tab hands over an empty path and no material is read.

`tobj` also answers a face with no `vt` with its neighbour's coordinate, so a hole is not
visible in the indices it returns.

## Decision

`UvMap` holds `Option<[Vec2; 3]>` per face. A face the file left unmapped does not move: the
press reads the field's own value there and subtracts nothing. A loader returns a map when
at least one face carries a coordinate, and `press` refuses only a map indexed by a
different face list, or one that covers no face at all.

Since `tobj` hides the hole, the OBJ loader reads an object as unmapped when its whole
coordinate set collapses to one point, which is what a part with no `vt` of its own comes
back as.

A `map_Kd` that resolves to nothing is tried again as a bare file name beside the model,
which is where such an image actually travels. An `mtllib` the parser could not split is
tried as the model's own name with an `.mtl` extension, which is where such a file sits.
`Texture::decode` reads BMP and TGA beside PNG and JPEG, since a model old enough to name a
`.bmp` is exactly the model that carries these paths.

## Consequences

The boundary between a mapped face and an unmapped one is a jump in the displacement, the
same seam ADR 0116 already names for a UV that jumps between two faces. It is now reachable
without a bad map, by a model that is simply part-textured.

The collapse rule reads a part deliberately mapped to a single pixel as unmapped. Such a
part would have moved by one constant distance, so what is lost is an offset, not a relief.

Both fallbacks can pick up a file that is not the one the material meant — a `relief.png`
beside the model when the material named another machine's `relief.png`. Nothing else in
the folder could have been meant, and the alternative is refusing the model outright.

`.dds`, which is what the same game models carry most often, is still refused: it is a
container of compressed blocks rather than an image format, and `image` does not read it.

## Alternatives considered

### Keep refusing a partial map

ADR 0115's rule, and it is safe. It lost because every real textured model out of a game or
an asset library is partial, so the rule refused the entire case the feature exists for.

### Parse the `f` lines ourselves to see which faces really carry `vt`

Exact, and it needs no heuristic. It lost because it means a second OBJ parser beside
`tobj`'s, which ADR 0111 chose precisely to avoid, and because the triangulation of a quad
would have to be reproduced to match face for face.

### The option that won, and what it costs

The collapse rule is a heuristic standing in for a fact the parser threw away. It is right
on every file we have, and it is not a proof; a future `tobj` that reports the hole would
let it be deleted.
