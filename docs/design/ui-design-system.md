# The window's design system

The tokens, the widgets made of them, and the layout they sit in. Why not stock egui:
ADR 0024; why the layout is fixed: ADR 0025.

Everything here is `crates/encrust-app/src/ui/`, and the rule the module exists for is:
**no colour, radius, gap or font size is written anywhere else in the crate.** It is
enforced, not asked for: `clippy.toml` bans every `Color32` constructor and `ui/theme.rs`
carries the one `#![expect]` that licenses them, while the renderer's constructors take a
token rather than floats so a raw colour does not type-check. See `docs/decisions/0107`.

## Tokens

`ui::theme` holds one `Palette` and a set of density constants, all derived from the three
colours in the logo — the plate `#151618`, the light face `#eee7e1`, and the layer gradient
`#ea7032` to `#be4925`.

### Surfaces, lightest last

| Token | Hex | Where |
|---|---|---|
| `sunken` | `#0f1012` | The viewport backdrop and the mask frame |
| `base` | `#151618` | Title and status strips |
| `panel` | `#1b1d20` | Inspector, tool rail, floating cards |
| `raised` | `#232629` | Inputs, unpressed buttons, list rows |
| `hover` | `#2b2f33` | Hover state |
| `hairline` | `#26292d` | Section dividers, panel edges |
| `line` | `#33383d` | Input borders |

A test asserts this ladder is monotonic in luminance: a surface is always lighter than the
one it sits on.

### Text and accent

`text_high` for values and headings, `text_mid` for labels, `text_low` for units and hints
and nothing else. One accent, with `accent_deep` pressed and `accent_wash` (14%) behind a
selected row or active segment; `on_accent` is the only thing drawn on an accent fill.
`ok`, `warn` and `danger` are functional — a machine that answered, a repair notice, a
mesh that cannot be oriented. The
axis colours are the CAD convention (X red, Y green, Z blue) and are used for the
transform letters and the gizmo, never for decoration.

Contrast is tested, not asserted: every text tone meets WCAG AA on `panel`, `on_accent`
on `accent`.

### The viewport

`theme::Scene` is the other half of the tokens: the grey of printed resin (`object`), the
cooler `support` and a `support_tint` per group after it, `selected`, `unsound`,
the translucent `blocker` and the red `trapped` the space resin cannot leave is painted
with, `painted` and `blocked`, the `section_cap` and the `section_wash` behind it, the
`overhang` a leaning face is marked with, the `outside` of what stands past the build
volume, the `gizmo` handles, and the plate's `grid`, `plate_border`, `volume` and two
`plate_axis` lines. It is separate from the `Palette` because none of it is window chrome
and the luminance ladder above does not apply.

`SEEN_THROUGH` is the one number beside them: how much of itself a surface keeps at most
when the viewport is drawing the models through, which is the x-ray view in `viewport.md`.

It reaches the GPU through `theme::gamma`, which is `Color32::to_normalized_gamma_f32` and
deliberately does **not** decode to linear: `egui-wgpu` writes gamma to a non-sRGB target
and our pipeline shares egui's `target_format`, so a decode here would wash the viewport
out. `ModelInstance::new`, `LineVertex::new` and `ExposureBand::new` take the token and
call it; the shader's two colours arrive as `Globals` fields.

### Type

Geist for text, Geist Mono for numbers, both compiled in from `assets/fonts/` so the
window looks the same on any machine. Three weights are named families: regular, Medium
for headings, SemiBold for section titles and the brand. Every millimetre, second, layer
index, triangle count and byte size is Geist Mono with tabular figures, without
exception — that is what makes a column of fields line up.

Phosphor is merged into the same `FontDefinitions` as a fallback, so an icon is a glyph in
an ordinary text run. `ui::icon` names the ones in use; a new icon is a name there, never
a codepoint pasted into a panel.

### Density

Row 28 points, input 26, panel padding 12, item gap 6. Tool rail 56, plate panel 228,
inspector 300, title strip 28 on macOS and 36 elsewhere, plate strip 30, status strip 26,
section rail 30 wide with a
4 point trough. The two side columns are the widths a drag on their edge leaves them at;
everything else is fixed. Controls are rounded by 6 points, surfaces by 8, and pills are
fully round. Nothing else is rounded.

## Widgets

`ui::widgets` holds what egui does not have. Each is a plain function of `&mut Ui` and
paints from tokens only.

| Widget | What it is |
|---|---|
| `section` | An inspector block: heading, optional hint, body, hairline under it; rows spaced `ITEM_GAP` |
| `section_with_action` | A `section` with one icon button in its heading, such as a reset |
| `subheading` | A group of fields inside a block, named in the quietest tone |
| `describe` | What a block or a setting is for, kept out of the panel and shown under the block's title when the title is clicked |
| `nested` | What a switch reveals, set in under it with a rule down its left edge |
| `list`, `list_row` | Rows a panel adds to — support groups, bands, holes — on one hairline |
| `card` | The frame of anything floating over the viewport |
| `stats` | Two columns of measured values, the cells separated by hairlines |
| `picker` | A row that opens a chooser: glyph, what is chosen, caret |
| `primary_button` | The one action a screen is for. There is never a second in view |
| `secondary_button` | The ordinary full-width button |
| `compact_button` | A button as tall as a field, to stand in a row of them |
| `icon_button`, `icon_toggle` | A borderless square with one glyph; the toggle stays lit while on |
| `tool_button` | A rail button, marked with an accent bar when it is the tool in use |
| `Segmented` | A pill of mutually exclusive choices, filled or washed |
| `switch` | A labelled toggle with an animated knob |
| `field_label` | The name of a field with its unit in brackets after it: `Gap (mm)` |
| `axis_label` | The same, the name an axis letter in its axis colour: `X (mm)` |
| `number_field` | The one box a number is typed or dragged in: mono, `FIELD_H` tall |
| `number_row`, `count_row` | A `field_label` on the left, a `FIELD_W` `number_field` on the right |

A number is always a `number_field` under a `field_label`; the unit is never written after
the box.

The filled `Segmented` is reserved for the one switch that changes what the whole window
is doing. Everything else uses the wash.

## Layout

`panels::Window::show` builds the window in a fixed order. Nothing is dockable.

```
┌───────────────────────────────────────────────────────────────┐
│ title:  File Edit View       [ Prepare | Preview ]      ⚙     │
├───────────────────────────────────────────────────────────────┤
│ plates 30:  Bench │ Brackets │ +          4 models, 2 selected │
├──────────┬─────────────────────────────┬────────────────┬─────┤
│ Plate    │                             │  Transform     │     │
│ contents │                     ╭ view ╮│                │rail │
│ 228      │      the stage      ╰──────╯│                │ 56  │
│          │                       ╭────╮│                │     │
│ ⧉ ⇋ ⊞ ≡ ␡ │                      │ 30 ││  ───────────   │     │
│ Printer  │                       ╰────╯│  Height Layers │     │
│ Resin    │                             │  Slice .goo  ⌄ │     │
├──────────┴─────────────────────────────┴────────────────┴─────┤
│ status 26                                                     │
└───────────────────────────────────────────────────────────────┘
```

The title strip is the window's own title bar rather than a band under one: it drags the
window, double-clicks to maximise, and on every platform but macOS draws its own window
buttons; see `docs/decisions/0104`. The plate's contents are a panel rather than a card
over the model, and the rail stands beside the panel it drives; see `docs/decisions/0102`.
Both side columns are dragged by their inner edge, and the plate folds away when that drag
goes past its floor, leaving a hairline that lights up and brings it back on a click. Neither is drawn in Preview, which
has nothing to edit, and the stage splits in two there — the model on one half, the
exposure mask on the other; see `docs/decisions/0103`.

What still floats over the stage is `egui::Area`s anchored to a rectangle by their corner,
laid out together in `panels/view_column.rs`: the view tools at the top right of the
viewport, and the section rail under them, one card width and centred in the height left.
In Preview the rail stands against the mask's edge instead, centred on the stage. The mode switch is not one of them: it sits in the title strip,
centred on the window, so it stays put whichever columns the mode draws. The section rail is the layer
scrubber, drawn in both modes and cutting the models at its own height; see
`docs/decisions/0061`. It is one column of controls wide, and the height it is parked at is
painted as a pill beside the handle only while the pointer is on the rail or for 1.4
seconds after it was last moved, so that a number does not sit over the model while nobody
is scrubbing.

Their rectangles are kept in `panels::Overlays` from the previous frame, because the
viewport reads the raw pointer before the cards are drawn and a press on a card must not
orbit the camera.

The inspector's footer is a bottom panel of its own rather than the end of the scroll, so
the slice action is always in the same corner whatever else is on screen. It carries the
stack's height and layer count over the button, and the output format on it; see
`docs/decisions/0105`.

## The Settings screen

One arrangement sits beside the plate's: with `Settings::open` the rail, the inspector and
the stage are not drawn at all, and everything between the two strips is the Settings
screen — a list of sections down the left, the form for the open section beside it. It is
a screen rather than a dialog because a profile is edited instead of the plate, not over
it, and because the sections keep growing. The gear at the right end of the title strip
turns into a close button while it is open. See `docs/design/profiles.md` and
`docs/decisions/0052`.

## Modes and tools

`workspace::Mode` is Prepare or Preview: the editing half of the application and the
reading half. Entering Preview cuts the plate into layers if the stack is stale or missing.
Both modes carry the section rail and cut at one height: a switch into Preview parks on
the layer nearest where Prepare was cut, and a switch back cuts Prepare at the layer shown.

`workspace::Tool` is what a click in the viewport does, and the rail groups the tools as
`PLACING`, `SHAPING` and `PRINTING`. Select shows the gizmo, the pick's bounds brackets and
a panel of position, rotation and scale together; see `docs/decisions/0118`.

The inspector shows the open tool's panel and nothing else; see `docs/decisions/0105`.
The machine and the resin stand at the foot of the plate panel, with a row of actions over
them — duplicate, mirror, array, arrange, remove — because both are the plate's and not the
tool's. The machine, the resin and the output format are remembered in `preferences.json`
beside the user's profile directory.
