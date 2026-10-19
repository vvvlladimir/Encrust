# 0312. Combine, label, cut and split models on the command line before slicing them

- **Status:** Accepted
- **Date:** 2026-10-19

## Context

The window combines models (ADR 0304), writes labels on them (ADR 0305, 0311), cuts them
with joints (ADR 0306) and splits them into parts (ADR 0307). `encrust slice` could do none
of it, so a script could not reach what a user can. `core-mesh-io` reads meshes and writes
none, so the command line has no file to hand a cut or a split back in; what it writes is a
sliced file, the way `--relief` and `--hollow` already change a model on its way there.

## Decision

`slice`, `estimate` and `batch` take a Modelling group of flags, applied to the models
after they are placed and before they are hollowed and supported: `--combine` joins every
model given into one; `--label TEXT --label-at X,Y,Z` writes on the surface nearest that
point, with the window's height, depth, sunk, follow and angle choices and an optional
`--label-font`; `--cut AXIS:MM` cuts at a plate coordinate, `--keep` picks the halves and
`--joint` joins them at the window's default sizes; `--split` and `--part-off X,Y,Z` break
a model up. A half that no longer reaches the floor is laid on its cut face, and a cut asks
for `--arrange`, which spreads the pieces. The work is the core crates' own calls; the
cut and its joint go through `core_joint::cut_and_join`, which the window takes too.

`encrust-cli` takes `core-joint` and `core-text` for it. A piece of several out of one model
is not hollowed or supported, since every report is one a model, and a project refuses the
flags: its models are shaped in the window.

## Consequences

Every modelling tool of the window is reachable from a script, at its default sizes, and a
cut plate prints in one run. The pieces cannot be taken out as meshes; reopen this when a
mesh writer lands, as subcommands writing STL. Joint sizes other than the defaults and
hollowing a cut model need a plate file of the pieces until then.

## Alternatives considered

### Subcommands writing meshes

`encrust cut a.stl -o halves/` would hand the pieces back to any program, but needs an STL
or 3MF writer in `core-mesh-io` first, a format of its own with its own tests, and the user
asked for the smaller step.

### The option that won, and what it costs

One more group of flags on an already long `slice --help`, and modelling that only ever
ends in a sliced file. A half laid on its cut face is turned about the plate's origin, so
`--arrange` is required rather than optional for a cut.
