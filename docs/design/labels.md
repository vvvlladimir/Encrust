# Labels

Text raised out of a model or sunk into it, where the user clicks. `core-text` outlines
and stands up the letters; `core_volume::combine` joins them to the model (ADR 0304, 0305).

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

That is a flat label. One that follows the surface is filled finer first: the
triangulated shape has every edge longer than 1/160 of the label's longest side, and never
under 0.25 mm, split at its middle, round after round, each triangle into the two, three or
four pieces its split edges leave. A split edge is split for both triangles on it and put
back into the ring, so the walls meet the caps and the letter stays closed. Under every
point a ray goes down the normal from the label's plane; if the face it meets looks down,
the point is inside the model and the ray goes up instead. That height, if within half the
label's longest side or twice its depth, is where the point's bottom and top are measured
from — the overlap below and the depth above for a raised letter, the other way round for
a sunk one — and a point with none takes the mean of the others. The walls stay square to
the label's plane, so on a steep face a letter is thinner across it by the cosine of the
slope (ADR 0311).

## Joined exactly

A label is joined with `combine_exact`, never through the field: a field remeshes the
whole model at its lattice, which rounds every edge of it for the sake of a few letters.
A model the exact cut cannot read is refused with `NotSolid`, and left as it was.
Only the side of the model the letters stand on goes through the engine: it is cut off
0.5 mm under them square to the clicked normal, joined, and stitched back along the cut
(ADR 0312; `docs/design/csg.md`).
