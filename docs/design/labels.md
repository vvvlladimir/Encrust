# Labels

Text raised out of a model or sunk into it, where the user clicks. `core-text` outlines
and stands up the letters; `core_volume::combine` joins them to the model (ADR 0303, 0304).

## From a string to shapes

Each character is looked up in the face and outlined by `ttf-parser`, the pen moving on by
the glyph's advance and the `kern` pair before it. Capitals stand the height asked for:
the scale is that height over the face's capital height, its ascender where it states
none. Lines stand a line height apart — ascender less descender plus the gap — each centred,
and the whole block is centred on the origin.

Curves are cut into straight pieces: a curve whose control polygon is `L` long gets
`sqrt(L / tolerance)` of them, at most 64, since a piece's sag falls with the square of how
many there are. The tolerance is 2 µm, a tenth of the finest pixel.

Fonts wind their outer rings either way round — TrueType clockwise, CFF anticlockwise —
so the winding is not trusted. A ring inside an even number of others is filled, inside
an odd number it is a hole in the smallest ring around it at one level up. Every filled
shape is turned anticlockwise with its holes clockwise, and each becomes its own prism, so
two letters that overlap are two solids the union folds together.

## On the model

A `Frame` is the clicked point, the face's normal, and up: the plate's z with the normal
taken out, or its y on a face within 8° of level, turned by the angle asked for. From every
corner of every letter a ray goes down the normal, starting the depth above the label's
plane; where it meets the model within the depth either side of the plane is how far the
face stands off it there. A hit further off — a pillar beside the label, the far side of a
thin wall — is not the face being written on and is passed over: reaching for it is what
cut a label into everything around it. A raised label runs from the lowest of those, less
0.2 mm, to the highest, or the plane if higher, plus the depth; a sunk one from the lowest,
or the plane, less the depth, to the highest plus 0.2 mm. So no letter ever reaches more
than twice the depth off its plane, and each overlaps the model on the other side, so that
no face of a letter lies on one of the model's.

The letters are flat. Text that follows a curved surface is not done.

## Joined exactly

A label is joined with `combine_exact`, never through the field: a field remeshes the
whole model at its lattice, which rounds every edge of it for the sake of a few letters.
A model the exact cut cannot read is refused with `NotSolid`, and left as it was.
