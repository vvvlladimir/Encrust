# 0291. The window orients for auto, at the model's own lift, and remembers the raft

- **Status:** Accepted
- **Date:** 2026-10-16

## Context

ADR 0282 left the window's goal at the fewest supports each time it opens, and ADR 0279
judges a pose on supports lifted by the profile's own lift. Once auto balanced every goal
(ADR 0284) it was the goal most models wanted, and a user who typed a lift into Position
saw auto-orient judge, and then stand, the model at a height they had not asked for. The
raft, set on the plate or on a model, was also lost each time the window closed.

## Decision

The window's goal starts at `OrientPreset::Auto`; it still lasts the session and is not
in `ToolSettings`. A model's `SceneObject::lift_mm`, set from **Lift off the plate**,
replaces the profile's `z_lift_mm` in what auto-orient searches, proves and stands the pose
at; nought or **On the plate** forgets it, and the profile's lift comes back. It lasts
the session and is not written to a project. The raft last set, on the plate or on a model,
is kept in `Preferences::raft` and is the plate's raft when the window opens. The command
line keeps the fewest supports as its default and the profile's lift.

## Consequences

The window and the command line now orient for different goals by default, and the docs
say which is which. A pose is judged at the height the model will print at. A preferences
file written before this reads with no raft and keeps the profile's. A user who wants the
lift to follow a project is the signal to write it into the manifest.

## Alternatives considered

### Keep the fewest supports as the window's default

It matches the command line, but a model auto would turn differently is left to a goal
that ignores the pull and the height.

### Write the lift and the raft into the project

They would follow a plate between machines, but change the `.encrust` manifest for a
choice made while arranging.

### The option that won, and what it costs

Two defaults to keep in step in the docs, and a lift that is gone when the window closes.
