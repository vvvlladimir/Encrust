# The window's design system

The tokens, the widgets made of them, and the layout they sit in. Why not stock egui:
ADR 0024; why the layout is fixed: ADR 0025.

Everything here is `crates/encrust-app/src/ui/`, and the rule the module exists for is:
**no colour, radius, gap or font size is written anywhere else in the crate.** It is
enforced, not asked for: `clippy.toml` bans every `Color32` constructor and `ui/theme.rs`
carries the one `#![expect]` that licenses them, while the renderer's constructors take a
token rather than floats so a raw colour does not type-check. See `docs/decisions/0107`.

## Tokens

`ui::theme` holds one `Palette` and a set of density constants: graphite surfaces, one ember
accent, and colours kept for state. Why these values and this face: ADR 0216.

### Surfaces, lightest last

| Token | Hex | Where |
|---|---|---|
| `sunken` | `#0c0e11` | The viewport backdrop, the mask frame, and the box a value is typed in |
| `base` | `#111418` | Top bar, tool rail, layer strip, the plate's summary |
| `panel` | `#161a1f` | Inspector, plate column, floating cards |
| `raised` | `#1e2329` | Unpressed buttons, list rows, the tool in use |
| `hover` | `#272d34` | Hover state |
| `active` | `#313840` | A pressed control, the track of a switch that is off |
| `hairline` | `#252a31` | Section dividers, panel edges, a box's border at rest |
| `line` | `#39414b` | A box's border under the pointer, rules |

A test asserts this ladder is monotonic in luminance: a surface is always lighter than the
one it sits on.

### Text and accent

`text_high` for values and headings, `text_mid` for labels, `text_low` for units, hints and
the names of readings. One accent, with `accent_deep` pressed, `accent_soft` for the accent as
text or a glyph — the tool in use, the active segment, a picked row — and `accent_wash` (14%)
behind an active segment; `picked_wash` (10%) is the selection. `on_accent` is the only thing
drawn on an accent fill.
`ok`, `warn` and `danger` are functional — a machine that answered, a repair notice, a
mesh that cannot be oriented. The
axis colours are the CAD convention (X red, Y green, Z blue) and are used for the
transform letters and the gizmo, never for decoration.

Contrast is tested, not asserted: every text tone meets WCAG AA on `panel`, `on_accent`
on `accent`.

### The viewport

`theme::Scene` is the other half of the tokens: the cool resin of the model (`object`), the
warm grey `support` and a `support_tint` per group after it, `selected`, `unsound`,
the translucent `blocker` and the red `trapped` the space resin cannot leave is painted
with, `painted` and `blocked`, the `section_cap` and the `section_wash` behind it, the
`overhang` a leaning face is marked with, the `outside` of what stands past the build
volume, the `gizmo` handles, the plate's `grid_minor` and `grid_major`, the translucent
`plate_border` and `volume`, the `plate` the plate is filled with, the dashed `section_frame`, the three `plate_axis` arrows, the
`shadow` the models cast, and the `light` they are lit by: a sky and a ground, a key and a
fill, with the directions the two lights travel. It is separate from the `Palette` because
none of it is window chrome and the luminance ladder above does not apply.

`SEEN_THROUGH` is the one number beside them: how much of itself a surface keeps at most
when the viewport is drawing the models through, which is the x-ray view in `viewport.md`.

It reaches the GPU through `theme::gamma`, which is `Color32::to_normalized_gamma_f32` and
deliberately does **not** decode to linear: `egui-wgpu` writes gamma to a non-sRGB target
and our pipeline shares egui's `target_format`, so a decode here would wash the viewport
out. `ModelInstance::new`, `LineVertex::new` and `ExposureBand::new` take the token and
call it; the shader's own colours and the lights arrive as `Globals` fields.

### Type

IBM Plex Sans for text and figures, IBM Plex Mono for what is read as code — an address, a
file name, a report — both compiled in from `assets/fonts/` so the window looks the same on
any machine. Three weights of the sans are named families: regular, Medium for headings,
SemiBold for section titles. Every millimetre, second, layer index, triangle
count and byte size goes through `theme::figures`: Plex Sans draws its digits at one width,
which is what makes a column of fields line up without a mono face.

Phosphor is merged into the same `FontDefinitions` as a fallback, so an icon is a glyph in
an ordinary text run. `ui::icon` names the ones in use; a new icon is a name there, never
a codepoint pasted into a panel.

### Density

Row 30 points, field 28, a named field's box 128 wide, button 32, primary button 40,
section heading 36, panel padding 12, item gap 8. Tool rail 68 with 46 point buttons, plate
panel 272, plate strip 44, inspector 328, top bar 44, layer strip 52, a chip in either 32.
The two side columns are the widths a drag on their edge leaves them at; everything else is
fixed. Controls are rounded by 6 points, cards by 8, windows by
10, and pills are fully round. Nothing else is rounded.

## Widgets

`ui::widgets` holds what egui does not have. Each is a plain function of `&mut Ui` and
paints from tokens only.

| Widget | What it is |
|---|---|
| `section` | An inspector block: a caret and title that fold it, optional hint, body, hairline under it; rows spaced `ITEM_GAP` |
| `section_with_action` | A `section` with one icon button in its heading, such as a reset |
| `subheading` | A group of fields inside a block, named in the quietest tone |
| `describe` | What a block or a setting is for, kept out of the panel and read from a question mark beside the block's title |
| `nested` | What a switch reveals, set in under it with a rule down its left edge |
| `list`, `list_row` | Rows a panel adds to — support groups, bands, holes — on one hairline |
| `card` | The frame of anything floating over the viewport |
| `later` | A control the design has and the window cannot do yet: greyed out, "Not available yet" on hover |
| `readings`, `stats` | Measured values, one to a row: name, dotted leader, figure |
| `notice` | A verdict: a glyph in its colour, the verdict in plain ink, what to do under it |
| `picker` | A row that opens a chooser: glyph, what is chosen, caret |
| `primary_button` | The one action a screen is for. There is never a second in view |
| `secondary_button` | The ordinary full-width button |
| `compact_button` | A button as tall as a field, to stand in a row of them |
| `icon_button`, `icon_toggle` | A borderless square with one glyph; the toggle stays lit while on |
| `rail_button` | A rail button, a glyph over the tool's name, raised in `accent_soft` when it is the tool in use, with a dot while the tool needs attention |
| `Segmented` | Mutually exclusive choices in one bordered box, split by hairlines, the chosen one washed |
| `switch` | A labelled toggle with an animated knob |
| `field_label` | The name of a field with its unit in brackets after it: `Gap (mm)` |
| `axis_label` | The same, the name an axis letter in its axis colour: `X (mm)` |
| `number_field` | The one box a number is typed or dragged in: sunken, `FIELD_H` tall, the figure at its right |
| `number_row`, `count_row` | The name on the left, a `FIELD_W` box on the right with the unit inside it after the figure |

A named number is a `number_row`, its unit inside the box in `text_low`; a box standing under
a `field_label` of its own carries the unit in that label's brackets instead.

## Layout

`panels::Window::show` builds the window in a fixed order. Nothing is dockable.

```
┌──────────────────────────────────────────────────────────────────────────┐
│ top 44:  ● ● ●  File Edit View          printer / resin │ 50 µm   Find     │
├────┬───────────┬──────────────────────────────────┬───────────────┬──────┤
│ 1  │ Models  + │ ╭view╮    ╭ job, notice ╮        │ the open tool │ rail │
│ 2  │ list 272  │ ╰────╯                           │ 328           │ 68   │
│ +  │           │            the stage             │               │      │
│    │ ⧉ ⊞ ≡ ␡   ├──────────────────────────────────┤               │ ⌨    │
│ 44 │ Slice  ⌄  │ layers 52: views ‹ ▶ › ━━━●── 650│               │ ⚙    │
└────┴───────────┴──────────────────────────────────┴───────────────┴──────┘
```

The top bar is the window's own title bar rather than a band under one: it drags the
window, double-clicks to maximise, and on every platform but macOS draws its own window
buttons; on macOS the system's are moved onto its centre line. See `docs/decisions/0104`
and `0217`. The File, Edit and View menus stand at its left end. The chip at the right
names the machine and the resin, each opening its list, and the layer height and exposure,
which open the print settings.

The plates are a strip of numbered squares at the window's left edge, outside the plate
panel so they stay when it folds; a right click on one duplicates, removes or gathers the
selection onto it. The sheet of keys and the gear stand at the foot of the
rail.

The plate's contents are a panel rather than a card over the model, and the rail stands
beside the panel it drives; see `docs/decisions/0102`. Both side columns are dragged by
their inner edge, and the plate folds away when that drag goes past its floor, leaving a
hairline that lights up and brings it back on a click. At the foot of the plate column the
summary states the plate's figures, or the layer's own in Preview, over Slice; the caret
beside Slice holds every plate at once and the bound machine.

The layer strip along the foot of the stage is drawn in every view. It switches the view —
the model, the model beside its layer, the layer alone — steps and plays, and its track
moves the cut, carrying the exposure bands along its top, the cured area as a profile
either side of the rule and a tick at each risk; a click on the layer figure opens it for
typing, and Enter goes there. See `docs/decisions/0061` and `0217`.

What floats over the stage is `egui::Area`s anchored to a rectangle by their corner: the
view tools at the top left of the viewport (`panels/view_column.rs`), the view cube and
the home view at its top right (`panels/view_cube.rs`), and top centre of
the stage the running jobs with their Cancel, a sent file waiting to be started, and the
last message (`panels/stage_notice.rs`). A failure stays there until it is put away; any
other message stands for four seconds. There is no status line.

The viewport reads the raw pointer rather than its own `Response`, so what is drawn over
it would otherwise orbit the camera as well. None of these cards reports where it is:
whichever egui layer is under the pointer owns it, and the plate is only under the pointer
when that layer is the viewport's own; see `docs/decisions/0193`.

## The Settings screen

One arrangement sits beside the plate's: with `Settings::open` the rail, the inspector and
the stage are not drawn at all, and everything under the top bar is the Settings
screen — a list of sections down the left, the form for the open section beside it. It is
a screen rather than a dialog because a profile is edited instead of the plate, not over
it, and because the sections keep growing. The gear at the foot of the rail opens it,
and its own close button or Esc leaves it. See `docs/design/profiles.md` and
`docs/decisions/0052`.

## Modes and tools

`workspace::Mode` is Prepare or Preview: the editing half of the application and the
reading half, switched from the layer strip's views. Entering Preview cuts the plate into
layers if the stack is stale or missing. Both modes carry the layer strip and cut at one
height: a switch into Preview parks on
the layer nearest where Prepare was cut, and a switch back cuts Prepare at the layer shown.

`workspace::Tool` is what a click in the viewport does and what the inspector shows, in
every view. `Tool::RAIL` groups twelve of them as Place, Modify, Supports and Finish; the
print settings are a form the top bar opens. Position carries the gizmo and the transform
fields; Select only says what is picked. See `docs/decisions/0218`.

A click on a model that is not picked only picks it, whichever tool is in hand: the tool
works on what is picked, so aiming it somewhere else is its own click (ADR 0193).

The inspector shows the open tool and nothing else: a heading with its one fact, its
sections, and its action pinned to the foot; see `docs/decisions/0105` and `0218`. A
control the design has and the window cannot do yet is drawn by `ui::later`, greyed out.
The plate's row of actions — duplicate, array, arrange, remove — is the plate's and not a
tool's. The machine, the resin, the output format and every value a tool panel holds are
remembered in `preferences.json` beside the user's profile directory, and each of those
values answers undo; see `docs/decisions/0192`.
