# 0190. Paint the space that holds resin, and see the model through

- **Status:** Accepted
- **Date:** 2026-10-06

## Context

ADR 0189 runs a drainage check after every hollow run and every cut, and reports each
pocket of resin with a point inside it and what it holds. The window drew that point as a
ball of its own red.

Nothing came of it on screen. The pocket is inside the model, the model is opaque, and the
ball stood behind the wall: a user who was told that resin was trapped had no way to see
where, and the mark they could not see was a point rather than the space that fills. The
decision to drill was taken blind.

The space itself is already meshed. `hollow` extracts the cavity, flips it and appends it
to the model, so the shell a viewport is already drawing carries the boundary of exactly
the volume that fills with resin. A cavity runs to millions of triangles (ADR 0070), so a
second copy of it — on the heap or on the card — is not available.

## Decision

The viewport gains an **x-ray** view: a second pipeline with `depth_write` off and
`depth_compare` `Always`, so nothing hides anything and every surface along a view ray adds
its own wash. How much of itself a surface keeps is `theme::SEEN_THROUGH` at a rim turned
away from the camera and the shader's `XRAY_FLOOR` of that head-on. It is a view, not a
tool: the view card and the View menu toggle it, and a drainage check opens it on the turn
from no trapped resin to some.

When a check finds resin in a model, that model's **whole cavity is painted red**. It is
drawn out of the shell that is already on the card: `Hollowed` and `Shell` carry the face
range the cavity occupies, a `ModelDraw` names a face range, and `pieces` breaks a cached
mesh at the ends of every range drawn from it as well as at the card's ceiling. The draw of
the whole shell declares the cavity's range even while nothing is trapped, so the pieces are
cut for it on the first upload rather than on the frame the check comes back.

An instance says whether it is a surface or a volume, and that red is a volume: the space
between the two walls that bound it rather than either wall. No light shades it, the inside
wash does not lighten its far wall, and the x-ray does not wash it down, so what the eye
reads is the token laid down twice, once per wall.

## Consequences

Trapped resin is a space on screen rather than a sentence in a panel, and it is the space a
hole has to reach rather than a guess at where that hole goes. The marks ADR 0189 drew are
gone, and so are the Hollow panel's two buttons for drilling them; the holes are the Drain
tool's.

The red costs no geometry and no upload: it is a second draw of pieces already on the card,
and because the shell declares the range before anything paints it, the check coming back
does not re-cut a mesh of millions of triangles. It is the whole cavity, not the pocket
alone, so a cavity drained everywhere but one corner still lights whole. The signal to reopen is a user reading that as "none of this drains"
and drilling where nothing was trapped; the fix is the pocket's own bounding box out of
`TrapScan`, tested against in the shader.

The x-ray applies to the flat pass alone, so a relief still shows what it would press in.

## Alternatives considered

### Keep the ball, and make the model translucent around it

The cheapest change, and what the first cut did. It answers "a pocket is somewhere here"
and not "this much resin stands in this shape", which is the question a wall thickness and
a hole diameter are chosen against. A ball sized by volume also reads as a part of the
model rather than as the absence of one.

### Mesh the pocket out of the trap scan

Exact: the scan knows which runs on which layers belong to which pocket, so it could hand
back the pocket alone. It would have to keep those runs instead of streaming them away,
and the mesh is a cavity's worth of triangles built, held and uploaded a second time — the
wall ADR 0070 was written at, paid again for a picture.

### The option that won, and what it costs

Painting the whole cavity is an over-statement wherever a cavity is partly drained, and the
face range ties the viewport to how `hollow` lays its three parts out: a change to that
order silently paints the wrong faces. The integration test that reads the range back and
measures what it encloses is what holds that down.
