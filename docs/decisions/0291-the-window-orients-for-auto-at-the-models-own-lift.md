# 0291. The window orients for auto, at the model's own lift, on one plate raft

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

ADR 0282 left the window's goal at the fewest supports each time it opens, and ADR 0279
judges a pose on supports lifted by the profile's own lift. Once auto balanced every goal
(ADR 0284) it was the goal most models wanted, and a user who typed a lift into Position
saw auto-orient judge, and then stand, the model at a height they had not asked for. The
Raft tab edited the picked model's own raft (ADR 0253), and a model is nearly always picked,
so a raft set there was gone when the window closed and the plate's came back.

## Decision

The window's goal starts at `OrientPreset::Auto`; it still lasts the session and is not
in `ToolSettings`. A model's `SceneObject::lift_mm`, set from **Lift off the plate**,
replaces the profile's `z_lift_mm` in what auto-orient searches, proves and stands the pose
at; nought or **On the plate** forgets it, and the profile's lift comes back. It lasts
the session and is not written to a project. The Raft tab edits the plate's raft whether
a model is picked or not, and the plate's has one home, `ToolSettings::raft`, which the
preferences and a project already keep. A model's own raft is made only by a project that
carries one; with that model picked the tab edits it, and **Use the plate's raft** drops it. The command line keeps the fewest supports as its default and the
profile's lift.

## Consequences

The window and the command line now orient for different goals by default, and the docs
say which is which. A pose is judged at the height the model will print at. The plate
stands on the raft it was left on, or the one an opened project brings. A new raft for one
model alone is no longer something the window makes. A user who wants the
lift to follow a project is the signal to write it into the manifest.

## Alternatives considered

### Keep the fewest supports as the window's default

It matches the command line, but a model auto would turn differently is left to a goal
that ignores the pull and the height.

### Remember the raft last set, on the plate or on a model

The switch a user last touched comes back, but one model's raft then becomes every model's
on the next run, and it overrides the raft a project just opened with.

### Keep the Raft tab on the picked model

It keeps ADR 0253's raft per model, but a user who sets a raft and reopens finds it gone.

### Write the lift into the project

It would follow a plate between machines, but change the `.encrust` manifest for a
choice made while arranging.

### The option that won, and what it costs

Two defaults to keep in step in the docs, a lift that is gone when the window closes, and
no way to give one model a raft of its own from the window.
