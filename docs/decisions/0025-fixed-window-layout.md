# 0025. Lay the window out in fixed panels and drop `egui_dock`

- **Status:** Accepted, scene card superseded by 0102
- **Date:** 2026-09-18

## Context

ADR 0002 chose `egui_dock` for the panel layout along with egui itself. Since then the
window has grown a viewport, a settings column, a scene list and a layer preview, and the
`DockState` has never been rearranged by anyone: it is built in `SlicerApp::default` as a
three-way split and used exactly that way. What the dock does contribute is a tab bar over
every panel, a crate that has to move in lockstep with every egui release, and ownership
of the rectangles, which is what a floating card over the viewport needs.

None of the slicers people come here from is dockable. They all put a fixed inspector
down one side, a tool rail down the other, and the model in the middle with cards floating
over it. The mockup this decision was taken against is that shape: a 44 point
title strip, a 56 point tool rail, a 300 point inspector, a 26 point status strip, and the
stage in what is left.

## Decision

The layout is fixed `egui::Panel`s, built in `panels::Window::show` in a fixed order: the
title strip on top, the status strip at the bottom, the tool rail on the left, the
inspector on the right, and the stage in the `CentralPanel`. `egui_dock` is removed from
the workspace.

The stage owns a second bottom panel, the layer transport, which is shown only in preview
mode, and the cards that float over the viewport are `egui::Area`s anchored to the
viewport rectangle: the plate contents at the top left, the view tools at the top right,
the printer badge at the bottom left, and the empty-plate invitation in the middle.

What the window is doing is now two pieces of state rather than which tab is focused:
`workspace::Mode` is Prepare or Preview, and `workspace::Tool` is what a click in the
viewport does. The gizmo no longer carries its own handle mode; it is told which handles
to draw by the tool.

This supersedes the `egui_dock` half of ADR 0002. The rest of 0002 — egui, eframe, and the
viewport inside an `egui_wgpu` callback — stands unchanged.

## Consequences

- One less crate to move on every egui bump, and no tab bar over a panel that was never
  going to be torn off.
- A card can float over the viewport, because the `CentralPanel` owns its own rectangle
  and an `Area` can be anchored inside it. That is what the scene card, the view tools and
  the plate badge are.
- The layer preview stops being a tab beside the viewport and becomes a mode over it, so
  the model and its layers are the same picture rather than two.
- The viewport reads the raw pointer, so it now has to know where the cards are: their
  rectangles are kept from the previous frame in `panels::Overlays`, and a press on a card
  is not an orbit. One frame of lag, which matters only on the frame a card moves.
- The layout is no longer user-configurable. Nobody can widen the inspector past its
  resize handle or move the rail. If a user ever asks for a second viewport or a
  tear-off preview, this is the decision to reopen.
- `app.rs` and `panels.rs` were rewritten and the two tests that asserted on `DockState`
  were replaced by tests on the opening mode and tool.

## Alternatives considered

### Keep `egui_dock` and restyle it

The tokens, the icon rail and the inspector grouping are all independent of who owns the
rectangles, so this would have worked. Rejected because it keeps the tab bars, keeps the
lockstep dependency, and leaves the floating cards fighting the dock for rect ownership —
paying a real cost for a rearrangeable layout nobody rearranges.

### Keep the dock but hide its tab bars

Cheaper than a rewrite. Rejected because it is the worst of both: the dependency and the
complexity stay, the user-visible feature that justified them is switched off, and the
rect ownership problem is untouched.

### The option that won, and what it costs

Fixed panels mean the layout is ours to maintain by hand, and every new surface has to be
given a place in `panels::Window::show` rather than dropped in as a tab. Users who like
rearranging their tools lose that, and there is no migration path back short of reverting
this ADR: the panels are now written as functions of the window state, not as dockable
tabs with their own identity.
