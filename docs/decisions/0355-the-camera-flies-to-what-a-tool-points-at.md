# 0355. The camera flies to what a tool points at, face on and round what is in the way

- **Status:** Accepted
- **Date:** 2026-10-26

## Context

The Repair tool marks its findings on the models (ADR 0354), but a hole of a few
millimetres on a 100 mm figure is a speck from where the camera stands, and a model of
seven holes and 2577 open edges gives no way to find them one after another. Check's
islands have the same need. The camera had one swing, the view cube's: a smoothstep over
0.35 s between two views, the target moving in a straight line at whatever zoom.

## Decision

- **Marks are one type for every tool:** `highlight::Highlight`, faces and edges each with
  its own colour, drawn in the order listed. A place to look at is a `Spot`: centre,
  radius, the side it faces.
- **`OrbitCamera::focusing` frames a spot** so a sphere of `radius / 0.35`, at least 3 mm,
  touches the frustum along the narrower side of the view — `r / sin` of the half angle, or
  `r / tan` without perspective — so the defect fills about a third of the view and what it
  lies in shows round it. It is seen from the side it faces; within 45° of that the angle is
  kept and only the target and distance move. A side another surface blocks, by
  `pick::occluded`, is passed over for rings tilted 30° and 60° round it, the one above
  first, then the current view if it is outside. It is never seen from behind its side,
  which is from inside the model: with every side blocked it takes its own.
- **Every swing moves along van Wijk and Nuij's path** (ρ = √2) for the target and the
  distance, the angles along the smoothstep; a flight takes 0.35 s per unit of the path's
  length, 0.35 to 1.2 s. The view cube keeps its 0.35 s.
- **Repair's rows visit their places in turn:** a press shows the finding and flies to its
  first place, each press after to the next, and the press after the last puts the marks
  away. The place in view is in `fault`, the others in `fault_rest`; the row keeps the
  count import found. A hole is a loop of open edges as repair would patch it, seen from the side its
  patch would face; faces come in pieces that share a vertex, seen from their summed normal.

## Consequences

- A finding is one press from being in view, and the next one is one press further.
- Home and the view cube's faces now pull back on the way when the target moves far; a turn
  in place is as before.
- Check's islands can be marked and visited with no new drawing or camera code: a
  `TODO(step-10)` stands in `highlight.rs`.
- A face whose normal points straight down is looked at from under the plate. Reopen if
  users find that disorienting; the plate's own side is the next candidate.
- The occlusion test is one ray from the spot's centre for each of up to fourteen sides:
  a spot half hidden behind a thin part is still flown to from its own side.

## Alternatives considered

### Keep the current angle and only frame the spot

The least motion, and the user keeps their bearings, but a hole on the far side of the
model is framed behind the model. Keeping the angle when it already faces the spot within
45° takes this where it works.

### Score many directions by visible area, as inspection planning does

Sampling views, ray tracing each and picking the best is what viewpoint planning for
inspection does, and it would see round a part one ray misses. It costs a ray cast per
sample of the spot per direction on every press; fourteen candidate sides and one ray each
answer on the frame.

### A straight line for the target, as before

Simple, but a long move at close zoom streams the model past faster than it can be read,
which is the case van Wijk and Nuij's path is derived to avoid.

### The option that won, and what it costs

A second interpolation in the camera, kept against the old one only by its two ends being
the views themselves, and a 3 mm floor and a 0.35 share that are judgement, not measure.
