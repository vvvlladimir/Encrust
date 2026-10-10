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
| `base` | `#111418` | Top bar, the foot of a side card: a tool's action, the plate's summary |
| `panel` | `#161a1f` | Every card over the stage: the rail, the inspector, the plate, the layer strip |
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
section heading 36, panel padding 12, item gap 8. Tool rail 60 with 46 point buttons, 34 without their names, plate
panel 272, inspector 328, top bar 44, layer strip 56, a chip 32.
The two side columns are the widths a drag on their edge leaves them at; everything else is
fixed. A card over the stage stands 8 from its edge and from the next card. A form block's title column is 180 wide beside at most 460 of fields; a window over
the plate is at most 1080 by 720, its title row 48 and its list 260; a table row is 36 and a
two-line row 48. Controls are rounded by 6 points, cards by 8, windows by
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
| `quiet_button` | A lesser action of a row with no surface until hovered, in `danger` when it cannot be taken back |
| `filter_chip` | One of a row of filters, bordered, washed in the accent while on |
| `dialog_frame`, `dialog_head` | A window over the plate: its frame, and its glyph, name and close button |
| `two_lines` | A name over the line that says more about it, in a list row |
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

`panels::Window::show` builds the window in a fixed order. Nothing is dockable. Under the
top bar the viewport takes the whole window, and every column is a card floating over it,
as tall as what it holds; see `docs/decisions/0221`.

```
┌──────────────────────────────────────────────────────────────────────────┐
│ top 44:  ● ● ●  File Edit View     printer / resin │ 50 µm   Find   ⌨  ⚙   │
├──────────────────────────────────────────────────────────────────────────┤
│ ╭────╮╭─────────────╮                       ╭cube╮ ╭────────────╮ ╭────╮ │
│ │rail││ the open    │   ╭ job, notice ╮     ╰────╯ │ 1  2  +    │ │view│ │
│ │ 60 ││ tool 328    │                              │ Models   + │ │ 650│ │
│ │    ││ action      │                              │ list 272   │ │  ▲ │ │
│ │    │╰─────────────╯                              │ ⧉ ⊞ ≡ ␡    │ │  ┃ │ │
│ │    │                                             ╰────────────╯ │  ● │ │
│ │    │            the stage, everywhere            ╭────────────╮ │  ┃ │ │
│ │    │                                      ╭view╮ │ This plate │ │  ▼ │ │
│ │    │                                      ╰────╯ │ Slice  ⌄   │ │  ▶ │ │
│ ╰────╯                                             ╰────────────╯ ╰────╯ │
└──────────────────────────────────────────────────────────────────────────┘
```

The top bar is the window's own title bar rather than a band under one: it drags the
window, double-clicks to maximise, and on every platform but macOS draws its own window
buttons; on macOS the system's are moved onto its centre line. See `docs/decisions/0104`
and `0217`. The File, Edit and View menus stand at its left end. The chip at the right
names the machine and the resin, each opening its list, and the layer height and exposure,
which open the print settings.

The tools are a rail at the stage's left edge with the inspector beside it: what is picked
opens next to where it was picked. The rail is always the stage's height; a tall screen
spreads its gaps up to a ceiling, a short one drops the names, then closes the gaps, and
only then scrolls. The plates are a row of numbered squares at the head of
the models card, which keeps only them when it folds; a right click on one duplicates,
removes or gathers the selection onto it. The sheet of keys and the gear stand at the top bar's right end.

A side card past the room it has scrolls its list — the tool's sections, the plate's
models — and keeps its foot in view (`ui::body_and_foot`). Both side cards are dragged by
their inner edge, and the plate folds away when that drag goes past its floor; a button
beside the plates brings it back. Under the models, standing on the stage's foot, a card of
its own states the plate's figures, or the layer's own in Preview, over Slice; the caret beside Slice holds
every plate at once and the bound machine.

The layer strip stands the stage's full height at its right edge, in every view, its top
the top of the print. It switches the view — the model, the model beside its layer, the
layer alone — reads the layer and its height, and its track moves the cut, with a layer
up over it and a layer down and play under it. The track carries the exposure bands down
its left, the cured area as a profile either side of the rule and a tick at each risk; a
click on the layer figure opens it for typing, and Enter goes there. The profile and the risks show in the model view too while
the stack is still the plate's. See `docs/decisions/0061` and `0217`.

Everything over the stage is an `egui::Area` anchored to a rectangle by its corner. Between
the side cards stand the view cube and the home view at the top right
(`panels/view_cube.rs`), the view tools at the bottom right beside this plate's card
(`panels/view_column.rs`), and top centre the running jobs with their Cancel, a sent file waiting to be started, and the last
message (`panels/stage_notice.rs`). A failure stays there until it is put away; any
other message stands for four seconds. There is no status line.

The viewport reads the raw pointer rather than its own `Response`, so what is drawn over
it would otherwise orbit the camera as well. None of these cards reports where it is:
whichever egui layer is under the pointer owns it, and the plate is only under the pointer
when that layer is the viewport's own; see `docs/decisions/0193`.

## The Settings screen, the Machine and resin window and the start page

Three arrangements sit beside the plate's. With `Settings::open` the rail, the inspector
and the stage are not drawn, and everything under the top bar is the Settings screen: its
pages down the left, the open page's list and form beside them. A form is blocks — a title in
a column of its own, the fields beside it, a hairline under them — and a title stands over
its fields where the form is too narrow for both. It is a screen because a support profile
is edited instead of the plate; the gear in the top bar opens it, and its close
button or Esc leaves it.

**Machine and resin** is a modal over the plate instead, because a machine and a resin are
picked for the plate in view: the machines down its left, the picked one's Resins, Machine
and Network tabs beside them, an action bar along the tabs' foot. A question or a
calculator over it is a modal of its own.

Until the first model, project or sliced file arrives, or a tool is picked, everything under
the top bar is the **start page** instead: a drop zone, the machine, and the files opened
lately. See `docs/design/profiles.md` and `docs/decisions/0052`, `0220`.

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
