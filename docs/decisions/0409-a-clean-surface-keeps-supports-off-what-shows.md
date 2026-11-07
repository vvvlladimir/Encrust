# 0409. Orient for a clean surface: supports off what shows, read from the shape

- **Status:** Proposed; the preset and its policy are superseded by 0410
- **Date:** 2026-11-06

## Context

Auto-orient can stand a figure face down. All its goals are resin, pull and time, and for
a figure the least resin is often the pose with every contact on the face. A person
avoids that by hand. Of the four goals, none can.

ADR 0222 had a clean top: it weighed the overhang on faces that look up in the file.
ADR 0284 removed it, because the file's +Z stands in poorly for what a person sees, so it
mostly kept the model as drawn. ADR 0277 named a brush that marks faces to keep clean as
what could say the rest.

The work on this is Zhang et al., *Perceptual models of preference in 3D printing
direction* (SIGGRAPH Asia 2015). They choose a direction that keeps supports off
perceptually significant regions. Their score combines support area, mesh saliency (Lee,
Varshney and Jacobs 2005), visibility from preferred viewpoints, and smoothness, weighted by
a user study. The common practice says the same: detail away from the plate, light tips on
faces.

## Decision

- **`core_geometry::Prominence`** reads how much each face shows, 0 to 1:
  `exposure × (1/3 + 2/3 × detail)`. Exposure is ambient occlusion through the `Bvh`.
  Detail is mesh saliency: centre-surround differences of mean curvature over six levels
  of a sparse grid of cubes, each scaled by what its top twentieth reaches. How it works is in
  `docs/design/prominence.md`. It lives in `core-geometry` because placement will read it
  too (step 3).
- **`OrientGoal::CleanSurface`**, "Clean surface", is a fifth preset. The search adds
  `Score::seen_mm2`: the plan area under supports, each face weighed by its prominence. It
  reads the goal as that area over one contact's patch. The proof reads
  `Printout::seen_contacts`: each placed contact weighed by the prominence of the face
  under it, found through a `Bvh` of the model. Both count one contact on open detail as
  one. Its weights are 1.0 on what shows, 0.2 on the resin, and the common height and
  footprint.
- **Its policy is banded**, like the least peel. It weighs cups, tearing and what hangs,
  and keeps within a tenth of the best. It has **enough** at one contact on show and falls
  back on the fewest supports.
- **Auto does not weigh it.** `OrientGoal::BALANCED` lists the three goals auto
  balances, and `ALL` keeps every reading. A goal added to `ALL` is no longer weighed by
  auto on its own. That changes ADR 0284's consequence: it now applies to `BALANCED`.
- **The prominence is read only where the goal is asked for**: in `Faces::for_settings`
  and in `prove`. `print_out` always reads it, so that `orient-lab` can judge every pose
  it measures. `orient-lab` judges the clean surface with the same enough.
- The window calls it **Surface** in the preset row (ru: Чистота), recipes and
  `--orient-goal` `clean-surface`.

Later steps, each its own change:

- **step 2:** a brush paints faces to keep clean, a `Region`, read as 1;
- **step 3:** support placement moves contacts within a pose off prominent faces, or
  makes their tips lighter there.

## Consequences

A figure printed for a clean surface stands with its face up and its contacts on the
back, under folds and on smooth areas. Clean surface keeps the face up on the 20 mm head
fixture, where the fewest supports lays it face down. Every other preset, auto included,
is unchanged.

The prominence costs about 0.3 s a million triangles, read once a run and carried on
`Printed` to the search and the proof; a hollow model's proof reads its shell's again.

The weights, the third, and the enough of one contact are set by eye, not by a corpus.
They are `orient-lab`'s to tune on figures. Reopen if a figure on the corpus keeps
contacts on its face under this goal, or if a technical part turns away from its fewest
supports for scars nobody sees.

Exposure treats every open direction alike. A face looking straight down at the plate,
such as the soles of a figure on a base, reads as open as its face. Only detail tells them
apart.

## Alternatives considered

### The file's +Z again, or learnt preferred viewpoints

The clean top read the file's +Z and was removed for it (ADR 0284). Zhang et al. learn
viewpoints from a user study we do not have, and Secord et al.'s viewpoint model asks for
the same.

### Saliency alone, or exposure alone

Saliency alone puts contacts on an open smooth back no worse than in a fold. Exposure
alone cannot tell a face from a smooth back: both are open.

### Recognising a face

Rendering views and running a detector would add a model and a dependency. It fails on
stylised figures, dragons and anything without a human face.

### Joining auto

ADR 0284 would weigh it with the rest at once. On a technical part nothing shows, so the
reading largely repeats the fewest supports, and a correlated goal would count support
twice in auto's product. It can join once `orient-lab` shows the weights hold on
figures and parts alike.

### The decision's cost

A fifth segment in Position makes each preset's segment narrower. The prominence is a
heuristic standing in for a perception study. It is a reading of shape, and it cannot
know that a cloak is meant to be seen while a chest is not.
