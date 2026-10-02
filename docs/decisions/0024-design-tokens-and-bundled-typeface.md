# 0024. Paint the window from one token module, with the typeface compiled in

- **Status:** Accepted
- **Date:** 2026-09-18

## Context

Until now `encrust-app` drew itself in stock egui: `Style::from_egui`, `ui.strong()` and
`ui.separator()` as the only hierarchy, `Move` / `Rotate` / `Scale` as text
`selectable_value`s, and every number set in the proportional UI font, so no two `DragValue`
columns lined up. The window had no relationship to the project's own logo, and a reviewer
comparing it to any other resin slicer could not tell which application was which.

The logo carries exactly three colours: the plate `#151618`, the light face `#eee7e1` and
the layer gradient `#ea7032` to `#be4925`. That is a palette, and a review of it derived a
full set of surfaces, text tones, one accent and three semantic colours from it, together
with a density and a type scale.

Two properties constrain how that palette can be applied. The application is offline: it
may not fetch a webfont, and it may not assume any face is installed on the machine. And
egui has no stylesheet: whatever is not in `egui::Style` has to be painted by hand, so
without a single place to read a colour from, the hex values spread through the panels.

## Decision

`encrust-app` gains a `ui` module that owns the whole design layer:

- `ui::theme` holds the one `Palette`, the density constants, the corner radii and the
  font sizes, and applies them to an `egui::Context` once at startup. No `Color32`,
  `CornerRadius`, `Margin` or `FontId` is constructed anywhere else in the crate.
- `ui::fonts` compiles Geist and Geist Mono into the binary with `include_bytes!` and
  registers them as the proportional and monospace families, plus named families for the
  Medium and SemiBold weights. Every number, dimension, layer index and triangle count is
  set in Geist Mono; the rest is Geist.
- `ui::icon` names the Phosphor glyphs the window uses. `egui-phosphor` merges the icon
  font into the same `FontDefinitions`, so an icon is a glyph in an ordinary text run
  rather than a second widget or a texture atlas of our own.
- `ui::widgets` holds the controls egui does not have: the section block, the floating
  card, the segmented control, the switch, the tool button, the axis field rows, the stat
  grid and the two text buttons. Each is a plain function of `&mut Ui`.

The fonts live in `assets/fonts/` with their SIL OFL licence beside them.

## Consequences

- A panel is a composition of named widgets, not a pile of `ui.label` calls with colours
  in them. Restyling is a change to one file.
- The binary grows by about 530 kB of font data and the Phosphor subset. For an offline
  desktop tool that is the right trade: the window looks the same on a machine with no
  fonts installed as on the developer's.
- Contrast is testable, and is tested: `ui::theme` asserts that every text tone meets WCAG
  AA on the panel surface and that the accent carries its own label.
- Anything egui draws itself — the menu, the slider, the progress bar — has to be talked
  into the palette through `Visuals`, and a control egui cannot be talked into has to be
  painted by hand. The switch, the segmented control and the tool rail buttons are each
  30 to 80 lines of painter code that a retained toolkit would have given us.
- `egui-phosphor` is now a lockstep dependency: it pins `egui ^0.36` and has to move with
  every egui bump, like `egui-wgpu` and `transform-gizmo-egui` already do.
- If the palette ever needs a second theme, `theme::apply` is where a light variant would
  branch. Nothing else in the crate would change.

## Alternatives considered

### Keep the stock egui theme and only fix the layout

Free, and no new dependencies. Rejected because the review's first finding was that the
window has no visual identity at all, and layout alone does not give it one. The numbers
still would not line up without a mono face, and the tool rail still would not exist
without an icon family.

### Depend on `re_ui`, Rerun's design layer

It is the existence proof that egui reaches professional polish, and it is readable
source. Rejected as a dependency because it carries Rerun's palette and its own widget
conventions, and it tracks Rerun's egui version rather than ours. We read it; we do not
link it.

### Load the fonts from the system by family name

No binary growth. Rejected because it is exactly the kind of assumption an offline tool
cannot make: the window would silently fall back to whatever is installed, and the density
and alignment the design depends on would differ per machine.

### The option that won, and what it costs

A hand-written token module and hand-painted widgets mean we own the maintenance. Every
egui release that changes `Visuals` or `Painter` is a file to revisit, and every new
control is painter code with its own hit testing rather than a line of markup. The tokens
are also only as good as their discipline: nothing but review stops a future panel from
writing a raw `Color32`, because the compiler cannot forbid it.
