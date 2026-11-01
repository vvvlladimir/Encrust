# UI standards

How the window must behave; [`ui-design-system.md`](ui-design-system.md) catalogues the tokens,
widgets, their states and the glyphs, and [`ui-screens.md`](ui-screens.md) lays out every
screen (ADR 0382). Where the window disagrees with any of the three, the window is wrong:
known gaps are at the end, each closed in its own commit.

## Principles

1. **A front end only.** The window takes input and draws; the work is a core call the
   command line makes too (architecture rule 9).
2. **One thing, one look.** Each kind of control is one widget in `ui::widgets`, painted from
   `ui::theme`. Panels compose widgets: no painting, no stock egui control, no styled label.
3. **Quiet by default.** Names, figures, one action. Meaning and mechanics sit behind `?`.
4. **Nothing is lost.** Every value is remembered between runs; every change to the plate or
   a tool is one undo away.
5. **Never frozen.** Work longer than a frame runs off the drawing thread, with progress and
   Cancel.
6. **Moves only to show a change.** A fold, a window, a notice, a pressed or armed control
   animates from `theme::motion`; nothing moves for decoration, and nothing at all with
   *Reduce motion* on.

## A tool panel

| Part | Content |
|---|---|
| Heading | Glyph, name, `help_mark`; at the right either the tool's modes (pick, add, paint) as `icon_toggle`s, or one fact summing up its state (the wall, the picked model) |
| Preset | If the tool keeps presets (ADR 0226): one `picker` showing the preset or "Unsaved", save and delete `icon_button`s; save asks the name in a popup |
| Sections | `section`s ordered what it acts on → how → rarely changed, folded; a switch enabling a section sits in its heading |
| Foot | One `primary_button`, at most one other action — an icon one is a square as tall as the primary; the only text is why the action cannot run |

A row is `number_row`, `count_row`, `switch` or `Segmented`: name left, control right at
`FIELD_W`. Fields in a grid (Move, Scale) take a `field_label` over a full-width box.

A control the window cannot do yet is not drawn. One this printer or model cannot use is
disabled with one sentence why; the way to change that is offered once per panel.

## Text

- **Names:** sentence case, one to three words, no colon, no unit: `Contact spacing`.
- **Explanation** — what a setting is for, how it works, when to change it — is a `describe`
  inside its `section`, read from `?`. Outside a section `describe` prints, so it is not
  called there.
- **Printed text** is only why something cannot be used, a verdict (`notice`), or a one-line
  empty state (`Nothing picked`). How to work the viewport is the `hint_bar` over the stage —
  `Click` place, `⇧ click` a line — never a paragraph in a panel.
- **Figures in a grid** stand under their name, the name cut with an ellipsis in a long
  language and the figure never wrapped (`figure_grid`).
- **Tooltips:** every icon-only control has its name and key; an action's says what it does.
- **Voice:** a resin printer user's words, actions as verbs, no internals, no "please" or "!".
- **Typography:** `×`, `µm`, `°`, `–` for ranges; figures through `theme::figures`.
- **Length:** a card is its column's width in every language; a name longer than its room
  is cut with an ellipsis and read whole on hover, never widening the card.

## Numbers

One unit and one number of decimals per quantity, everywhere — field, reading, chip, tooltip:

| Quantity | Unit | Decimals |
|---|---|---|
| Position, size, distance, wall, spacing | mm | 2 |
| Layer height | µm | 0 |
| Angle | ° | 1 |
| Exposure, wait | s | 2 |
| Lift and retract speed | mm/min | 1 |
| Resin volume / part volume | ml / mm³ | 2 / 1 |
| Ratio, scale / multiplier | % / × | 1 / 2 |
| Pixels, layers, counts | px, none | 0 |

The unit stands in the box after the figure, or in the brackets of a box's own `field_label`;
never both, never neither. A typed value is clamped to its range; no error popup.

## Undo (ADR 0086, 0192, 0268)

- **Undoable:** every change to the plate and every tool value; one gesture is one entry,
  recorded on a settled frame (no button down, no field focused).
- **By snapshot:** a value is undoable because it lives in the scene's inputs or in
  `ToolSettings`. A new value anywhere else is not, and that is a bug.
- **Not edits:** choosing a printer or resin (a new baseline), opening a project, a mesh a job
  lands, selection, camera, folds, panel widths.
- In a profile form, undo goes to that form only.
- **The irreversible asks first**, by a second press on the same control (`confirm_button`,
  `bin_button`): deleting a profile, a machine or a preset, taking a resin off a machine,
  removing a support group or every support, Stop on a running print. Renaming or
  overwriting a profile and closing unsaved work ask in a modal. Anything else undo takes
  back acts at once and never asks.

## What is remembered

| Belongs to | Home | Carried by |
|---|---|---|
| The plate: a tool value | `ToolSettings` | project, `preferences.json`, undo — for free |
| The window: machine, resin, behaviour, widths, a panel's mode | `Preferences` | `preferences.json` |
| A widget: fold, tab, lock | `ui::kept` by id (ADR 0334) | `preferences.json` |
| A named set: machine, resin, preset | profile catalogue | the user's profile directory |

Everything not transient (hover, drag in progress, one-shot action) is remembered; never in a
new file or egui memory alone. A missing or unreadable value opens at its default silently;
an unknown one is ignored. A setting applies as it changes: no Apply, no OK.

## Feedback

- A job's progress and Cancel stand in the notice stack (ADR 0263); a failure stays until put
  away, anything else four seconds. A notice is coloured by its glyph and action only, never a
  stripe. A modal only asks what a second press cannot: closing unsaved work, renaming or
  overwriting a profile, giving up the profiles that would not read.
- A value the window changed for the user is a `carried_row`, in `warn`, with the way back.
- Every action is an `Action` in the key table, so menu, Find, tooltip and key sheet know it.
  Every painted control names itself and takes focus (ADR 0229); Esc closes the topmost.

## Adding to the window

Its screen in `ui-screens.md` (a new part or state written there first, with its size and
place) → widget from the catalogue, in every state of the matrix (or a new one with its row)
→ glyph from the glyph table → text in `en.ftl`, help in `describe` → value in its home
above → the same in `encrust-cli` → panel pictures before and after (ADR 0215).

## Known gaps

- Decimals: a unit of the table brings its own (`field.rs`, `standard_decimals`); `%`
  and units the table lacks (g/ml, the compensation's) still take the caller's.
- Stock egui menus: the printer list the plate card opens, a model's import popup, Add a
  resin.
- Stock egui controls in `title_bar/menus.rs`, `settings/printer/connection.rs`,
  `scene_panel.rs`, `settings/machines/resins.rs`, `plate_strip.rs`, `project.rs`,
  `profiles.rs`; ~120 bare `ui.label` calls outside `ui::widgets`, most in `stage_notice.rs`,
  `inspector/check.rs` and `settings/`.
