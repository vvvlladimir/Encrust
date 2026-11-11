# The window's screens

What each screen holds, where and how big: the layout every change is held to (ADR 0382).
The tokens and widgets named here are [`ui-design-system.md`](ui-design-system.md)'s; how a
panel behaves is [`ui-standards.md`](ui-standards.md). Sizes are points at 1280 by 800; a
card stands 8 from the stage's edge and from the next card unless a row says otherwise.

## The frame

```
┌ top bar 44 ─ menus ─────── [ring] [machine ▾ | resin ▾ | 50 µm · 2.20 s] ─── Find ⌘K  ⌨  ⚙ ┐
│ ┌inspector 316┐ ┌──────── tool row ────────┐ ┌models 272┐ ┌strip 64┐                         │
│ │ heading 48  │ └──────────────────────────┘ │          │ │ views  │                         │
│ │ preset      │      [hint bar]      [cube]  └──────────┘ │ layer  │                         │
│ │ sections    │                                           │ track  │                         │
│ │             │            viewport                       │        │                         │
│ │ foot        │       [notices]     [view tools] [prep]   │ play   │                         │
│ │             │                                  ┌plate┐  │        │                         │
│ └─────────────┘                                  └─────┘  └────────┘                         │
```

| Part | Where | Size |
|---|---|---|
| Top bar | the window's top, its own title bar (ADR 0104) | 44 tall |
| Inspector | the stage's left edge, from its top | 316 until dragged; as tall as its content, at most the stage |
| Tool row | over the stage, centred between 8 right of the inspector, shown or put away, and 8 left of the models card | as wide as its tools, at most that room; 58 tall |
| Models card | 8 left of the layer strip, from the stage's top | 272 until dragged |
| Plate card | under the models card, standing on the stage's foot | 272, grows upward |
| Prepare row | 8 over the plate card, in Prepare while the plate holds a model | as wide as the plate card; 32 tall, a run too |
| Layer strip | the stage's right edge, its full height | 64 |
| Hint bar | 8 below the tool row, kept 8 clear of the view cube on both sides: centred under the tool row while it names what each key does, the keys alone flush with the row's left when that does not fit, always beside the mask | as wide as what it says, at most that room; one line, 28 tall |
| View cube | 8 left of the models card, Home under it while the pointer is on it: centred on the tool row's height while 8 clear of it, never above the stage, else 8 under it | 80 square; Home 28, 4 under it |
| View tools | 8 left of the plate card, its foot level with the card's | a column of 26 squares |
| Notices | centred on the stage left of the layer strip, moved in to stay between the inspector and the view tools, 16 above the foot; in Preview the same, over both halves | at most 440 wide each and never wider than that room, 6 apart, newest nearest the foot |

Everything over the stage but the Prepare row is a `card` on `CARD_SHADOW`; nothing over the stage carries a
stripe or a border of its own.

## Top bar

Left to right: on macOS 72 for the system's buttons, File, Edit, View as quiet text
buttons; in the centre the printer ring (while a printer prints; idle ones are reached from the plate card) and the chip; at the right
the Alpha pill, Find (as wide as its word and `⌘K` at its right), the key sheet and the gear as 30
squares.

A menu of the bar is a `menu_row` list 264 wide on `raised` ringed in `line`, the open title
on `hover`. Rows 28, glyph column 18, keys right in `text_low`; a hairline between groups; a
row that cannot run is `text_low`, the first of its group with why under it. File: New plate,
New project · Open project, Add model, Open recent ▸ (files with their age, Clear the list),
Test prints ▸ (Crustling with Download, its percentage or STL as keys, then the four drawn
in code; a browser, the four alone) · Save, Save as · Slice to .ext (why when off), Slice
all N plates, Send a sliced file (where the printer in hand sends over a network; its name
in the tooltip, why when off) · Import ▸
(Machine profile, Resin profile), Machine and resin, Settings · Quit (not in a browser).
Edit: Undo, Redo · Select all, Deselect, Duplicate (why when off), Remove, Move to plate ▸
(the plate in hand ticked) · Arrange plate (why when off), Auto-orient plate, Prepare for print (why when off) · Find. View:
Frame view · CAMERA Perspective or Orthographic, the one in force ticked · SHOW Plate grid,
See through models, Show as printed, Depth of field when shown as printed, ticked when on ·
Keyboard shortcuts.

The chip is three parts in one bordered box 32 tall: printer glyph and machine with a caret,
flask glyph in the resin's colour and resin with a caret, sliders glyph and `layer height ·
exposure`, all three in `text_high` (the resin in `warn` while untuned). Each part is its own button and lights in `accent_wash` while what it opens is open. Under 1100 the
chip drops the maker; under 720 the right end is glyphs only.

The machine and resin parts each open a chooser hanging 6 under the chip from its part's
left edge, 300 wide, on `raised` with a window's shadow, a capital group name over its rows:

- *Your machines*: a 44 two-line row a machine — glyph, name over maker and output format in
  `text_low`, an `ok` dot when a printer it lists answers on the network (`text_low` when none does,
  none when it lists none), the one in hand washed and ticked; under a rule, *Add a machine*
  (the library).
- *On the machine's name*: a row a resin on that machine — its colour as a 10 point dot,
  name over type and layer height, its exposure at the right in `text_mid`, the one in hand
  washed and ticked; under a rule, *Add a resin*, opening Machine and resin on the
  machine's resins.

Choosing a row takes it in hand and closes the chooser, as Esc or a click elsewhere does; a
footer row opens Machine and resin on that machine's page or that resin's form.

The ring is 32 across, filled clockwise with the current print's fraction in the colour of
what the printer is doing, the percent in its middle and a count badge when more than one
printer answers.

## Tool row

Nine tools in three groups split by a vertical rule: Select, Measure | Repair, Hollow, Drain,
Cut, Relief | Supports, Check. Each is a `rail_button`: glyph 19 over its name in 10.5, 50
tall, the room shared equally but no wider than 56 (its height, or the least a name needs), 2 apart. The tool in hand is washed in
`accent_wash` with its glyph and name in `accent_soft`; a tool that needs attention carries
a 7 point `warn` dot at its top right; Check's takes the colour of its gravest issue, the
plate's the moment it has one, open or not, and goes once it is fixed. While the pointer is on the row each tool shows its
key's digit, 1 to 9, at its top left in `text_low`. Narrower than 9 × 56, the names go and
each tool is a 36 tall square of its glyph.

## Inspector

One tool's card, in this order; any part a tool lacks is not drawn.

| Part | Content | Size |
|---|---|---|
| Heading | glyph in `accent_soft`, name, `help_mark`; at the right either the modes as `icon_toggle`s in one sunken group, or one fact in `text_mid` cut with an ellipsis | 48 |
| Preset | `dropdown` showing the preset (and `· edited` in `warn` once a value differs), Save and Delete as 32 squares | 32, padded 10 × 12 |
| Sections | `section`s, ordered what it acts on → how → rarely changed | heading 36, body padded 0 12 12, rows 6 apart |
| Foot | on `base`: one `primary_button`, at most one companion; nothing under it, the reason it cannot run its tooltip | primary 40, padded 10 × 12 |

The body scrolls; the heading, preset and foot do not. What each tool holds:

| Tool | Heading right | Preset | Sections | Foot |
|---|---|---|---|---|
| Select | the picked model's name | — | Global / Local; Move (X Y Z, reset, drop to the plate); Rotate (X Y Z with −45° +45°, reset), both at zero under Local, a typed or dragged figure a step along the model's own axis that falls back to zero; Scale (Size and Ratio columns, Keep proportions, Scale to fit); Place (On the plate · Centre side by side, Lay on face, Mirror, Mirror a copy, Lift off the plate as a `number_row`); Auto-orient (Auto / Supports / Peel / Time; Limit tilt as a `switch`, off at first, and under it while on Max tilt as a `number_row`, °; Clean surface as a `switch_with_toggle`, its brush the toggle while it is on, and under it Brush as a `number_row` while it paints, Clear what shows once any is painted; the handles are put away while the brush paints; Auto-orient; running, `step_bar` with the step and Cancel) | — |
| Measure | modes Points, Planes | — | Snap to (Corner / Edge / Free, Show dX dY dZ, Measure along the surface); Reading (Along the surface first when on, or why there is none) | — |
| Repair | modes Whole model, Paint | — | Found (`issue_row`s with counts, readings, look again); Method; Settings; Shells | Repair; running, `step_bar` with the step and Cancel |
| Hollow | the wall | yes | Shell (Inside / Outside / Bottom open, Wall, Precision); Infill (switch in heading, not for a mould); Keep solid (Keep thin parts solid as a `switch`, off at first, and under it while on Thinner than as a `number_row`, mm; then, not for a mould, Fixed size / Fit to thickness, Past the wall under Fixed size, the window's whichever model is picked; a click places one, a shift-click a rod on from the last); Already done | Hollow |
| Drain | whether checked | yes | Trapped resin (a `notice`-style verdict); Opening (Hole / Channel, size, depth, taper, Round / Polygon); Plugs | Check drainage, companion Auto (`companion_button`, the auto glyph), always with a vent as the command line's default; running, `step_bar` with Looking for trapped resin or Looking for drain holes and Cancel |
| Cut | the plane and its height | — | Plane (X / Y / Z, Height); Afterwards (Keep both / Lower / Upper, Lay the halves flat, Join the halves → `nested` joint kind and its rows); Parts | Cut, companion Split |
| Relief | modes Texture, Text, Combine | — | per mode: Where, Image, Depth, Leave flat — or Text, Lettering | Press it in, or the reason |
| Supports | modes Pick, Place, Paint where, Shape | the group picker | Picked; Where, Clean surface last as in Select, the same switch — or in Shape, Part with its diagram | Grow the supports, companion bin (every support); nothing printed over it — once the trees have grown, its tooltip adds a warning line with its count for each reason supports were left out: no room for a tip, on a face that looks up, nothing to stand on |
| Check | the number of issues, the plate's with the layers', `fixing` while a run holds the plate | — | On the plate, before any layer is read: Placement (`issue_line`s: the model, and at its right outside the build volume in `danger`, in the plate inset in `warn`, or meets and the other model in `danger`; a press flies the camera there, on a meeting to its next place) with Move to free space and, each only with something to answer, Scale to fit, Combine; Hollow and drain (hollow with no drain hole, worth hollowing; a press picks the model and opens Drain or Hollow) with Drill drain holes, which stays in Check. In the layers, previous / next as arrows in its heading, stepping through every issue of the panel, the plate's first, and going round; a card a kind: Islands (Support them · Remove them; once removed a quiet `verdict_card`, then Support them · Remove next · Put the islands back), Suction cups (Drill them · Auto-orient), Weak necks (Support them · Auto-orient), Hard peel (its readings; Reduce peel, Lift settings on a printer that leaves the lift to the file), Thin sections (no fix, why in a `hint`); each row the layer and its reading in N, mm and MPa, mm², the kind's name on hover. Every kind is an `issue_card`, the one open chosen and remembered, the next opening when it is answered; its rows are `issue_lines`, five shown and the rest scrolling; its buttons `flow_buttons`, the first the suggested one. A kind with a fix made and nothing left is an `answered_row`, Undo on the latest edit alone; with something left, the card with what the fix said over all its buttons, the fix that had its turn among them (ADR 0420). While the stack is not read, the spinner and *Reading every layer* or how to have it cut; while only the quick check from the background is in, its issues on their cards, their buttons off, under them the spinner and *Found by a quick check. Reading every pixel* (ADR 0419); nothing found by the full check, the all-clear verdict. Peel, its readings as a section of their own where no layer pulls too hard | Fix all; running, `step_bar` with the step and Cancel. No foot with nothing it can answer |
| Print settings | `layer height · exposure` | — | Layers; Exposure; By height; Edge smoothing with its state at the right; Motion and waits | Use the profiles' settings |

Print settings is opened from the chip, not the row; the row lights no tool meanwhile.

### The support group picker

The preset `dropdown` shows the group in hand: its colour dot, its name, `· edited`, its
support count, caret. Open, it is a list over the sections, as wide as the card less 12
each side, on `raised` with a window's shadow:

1. **On the plate** — one row per group holding supports: colour dot, name, count; on hover
   (always while hidden) hide and a `bin_button`. The group in hand is washed, with no tick.
   Auto has no dot of its own; the shapes it built stand set in under it with theirs.
2. **Profiles** — the profiles holding nothing, in `text_mid`, no count.
3. A foot row under a rule, sliders, *Edit the profiles* and an arrow out at its right,
   opening Settings → Supports.

Rows are 32 tall, the dot at 10 in, Auto's shapes 20 further; from the right the 26 squares
of hide and bin, the count. The list hangs 6 under the box, as every dropdown's.

Choosing a row takes its profile in hand and closes the list. The preset's bin removes the
group in hand with its supports; the foot's bin every support; both ask twice.

## Models card

| Part | Content | Size |
|---|---|---|
| Plates | numbered 26 squares, the current washed in `accent_wash`; + | row 42 |
| Heading | *Models*, `· project name` in `text_low`, `n picked` in `accent_soft`, + | 36 |
| Rows | eye or eye-slash, name cut with an ellipsis, state tags from the right: `Broken` in `danger_wash`, then `Meets` in `danger_wash` naming the models it meets on hover, a click flying the camera to where they meet, the next click to the next place | 32; picked washed in `picked_wash`, no stripe |
| Actions | Duplicate, Array, Arrange, Remove as 26 squares, spread evenly | 34, hairline over it |

## Plate card

| Part | Content | Size |
|---|---|---|
| Heading | `card_title`: *Plate n* in `text_high`, the resin's name at the right in `text_low` | 36 |
| Verdict | a `warn` `notice` when a model on the plate is broken, 8 over the figures | as tall as its text |
| Figures | `figure_grid`: Time (`17m26s`, shrunk to fit its cell in a long language), Resin (ml), Weight (g), Price (the currency; `—` when the resin has none), each unit 6 after its figure | two rows of 46 |
| Details | `details_fold`, 8 under the figures: a `split_readings` table of Model / Supports for resin, weight and price, then Vertices and Triangles as `readings`; its caret points up closed and down open; open, its row stays washed in `raised` | 26 closed |
| Foot | the printer row, then the action row, nothing printed under it | on `base` |

The printer row is 34 tall on `raised`: printer glyph, name, a status dot and word (`ok`
Ready, `accent` breathing Printing n %, `text_low` Not answering), caret, and the open-sheet
square inside it at its right. A profile listing no printer has no row. The send primary
reads *Slice and send*: the printer is named in the row over it.

While Details opens the card keeps its foot still: egui places an area from last frame's
size, so a card held by its foot asks for the frame to be laid out again whenever it changes
size, a fold does the same once it has measured what it holds, a motion holds one value for
the whole frame, and a frame may take three passes (`panels::card_at`, `theme::motion`).

| State | Action row |
|---|---|
| No printer, or the profile sends on a stick | Slice to .goo as the primary |
| A printer ready | Slice and send primary, Slice to .goo as the 40 square beside it |
| Choosing | the printer row opens a list of printers, each with its state, and *Scan again* last |
| Sending | the slice then the upload named over its bar, its percent under it, Cancel under that |
| The file landed | Start print primary, the 40 square beside it |
| The printer busy | Slice to .goo primary, its tooltip *Bench A is printing a.goo; the file will be saved instead.* |
| Nothing to slice | Slice to .goo disabled, why in its tooltip |

A broken model's verdict is a `notice`: *1 model is broken*, *What is sliced from it may
not print.* What stands past the plate or meets another model is Check's, not the card's. In Preview the heading is *Layer*, the figures are Exposure, Time to here (the
print until that layer is done), Cured area and Volume of the layer shown, and there is no Details.

## Prepare row

The recipe as a `dropdown` with the prepare glyph and the play glyph as a 32 `companion_square`
beside it, 6 apart, bare on the stage. The list, `dropdown_menu_wide` as wide as the row:
*Built-in* then *Yours* under `group_name`s, a recipe that cannot run led by the warning
glyph, and under a rule *Edit recipes* as its `picker_foot` opening Settings → Recipes.
The square runs the recipe on what a tool would act on, its tooltip that or why it cannot,
the square off. While a run goes the picker's place is a `run_box`: the turning glyph, the step and the
share done, washed in `accent_wash` to it; the square is the cross, its tooltip *Cancel*.
Waiting (ADR 0407) the box says *Step 3:* and why — with the pause glyph in `accent_soft`
the pause's message (else *Prepare for print is paused*), or with the warning glyph in
`danger` and a `danger` edge why the step failed; the share stays; then End, the stop glyph, and Continue, the play
glyph, as 32 `companion_square`s, their tooltips what each does.

## Layer strip

Top to bottom, centred in its 64: the three views — model, model beside layer, layer — as
`icon_toggle`s in one `mode_column`, centred; the layer number in semibold 16 in a
sunken box 26 tall in `accent_soft` that a click opens for typing; `of n` in `text_low` and
the layer's height in mm in medium `text_mid` under it; layer up; the track;
layer down; play. The track is a 3 point rule, filled in `accent` between its two thumbs —
26 × 12, ringed in `accent`, the upper one the cut, the lower the first layer shown. Behind
the rule, every 2 points up it, the widest cured area there sets one filled silhouette, each row the widest within 8 points averaged over as many so
peaks run into each other, in
`text_low` at 35 % centred on it, at most 44 across, so the supports' base reads wide and
the model narrow.
Across the rule a bar marks each risk the check found, in the colour the Check tool gives
it: `danger` for one that will fail (an island, a part left floating), `warn` for one that
may (a thin section, a hard peel, a lever, a suction cup), 28 × 3, 32 × 5 under the pointer
with what it is and its layer as a tip; a press goes to it as the Check tool's list does. Scale ticks stand at the right edge in `line`. Nothing else is drawn on the
track. With no stack it reads *No layers*.

## Over the stage

- **Hint bar:** a pill on `base` at 88 % with a hairline, 12 point text: the key in
  `text_high`, what it does in `text_mid`, pairs split by a 12 point rule. Every key is
  Click or Drag, any modifier named before it (Shift click, Option drag), or Esc. Drawn only
  while the tool in hand has something the pointer does in the viewport (Measure, Hollow,
  Drain, Relief, Supports' Pick, Place and Area, Repair's faces and edges, Cut's part, the gizmo's Shift drag). Short of room, only the keys show, as glyphs where
  there is one (mouse click, hand grabbing, on a Mac ⇧ ⌥ ⌘), each naming what it does on hover.
- **Gizmo drag:** the amount by the cursor on a size label's plate, signed, in the axis
  colour (text colour for the circle); for a move, a 3 point dot at the start, a dashed line
  to the pivot, and the arrow's line through the start at 45 %. Size labels clear the rings.
- **View tools:** Frame the view, Plate grid, See through, As printed, Perspective, as 26
  squares in one card; the on ones washed.
- **Notices:** each a card 40 tall: glyph in its colour (`warn` warning, `danger`
  warning-octagon, a turning circle-notch for a job), the words, at most one action as a
  small button washed in the accent, and a close square. A job carries its `progress_bar`
  and percent and a quiet Cancel instead of the close. Measure's walk along the surface shows
  its card only once it has run 0.3 s, so a short one does not flash. Prepare for print's is
  *Preparing for print:* and its step, only in Preview, where the Prepare row is not shown.
  Drain holes taken out and plugs cut are one card: *Reading the model for its holes and
  plugs*, then *Cutting N plugs*, then *Drilling the holes*, at the share of whichever is least
  far along. Its Cancel leaves the holes cut approximately and the new ones without plugs, and
  says so; a model's plugs land, and say where, only once its holes are out.
  Waiting there, it is a notice instead: the pause glyph in `accent_soft` or the warning
  glyph in `danger`, what the row's box would say, *Continue* as its action and End as its
  close.
  No printer or no resin is a `danger` notice with the reason on a small line under it.
- **Empty plate:** nothing over the stage; the inspector shows its heading alone, and the
  models card says *Nothing on the plate* and *Drop a model into the window* under a 30
  point cube-focus glyph, with Open a model as an inline button. Slice's reason is its tooltip.

## Preview

The stage splits at its middle (ADR 0103): the model cut at the layer on the left, the mask
on black on the right, a pixel grid under it. At the mask's top right, 8 left of the layer
strip and level with its top, over the side cards too, zoom out, the zoom in percent, zoom in and *show the whole
mask* in one card, and under it, flush with its right, a tag for the panel's pixels. The models card gives way, the plate card staying
on the stage's foot, the view tools 8 left of it over the mask as in Prepare. Beside the mask the hint bar shows the keys alone.

## Windows over the plate

A window is on `panel` with a 10 radius and a `line` ring, over a scrim of the stage at
62 %, at most 1080 by 720 and 24 from the window's edge; its title row is 48 — glyph, name,
close.

**Settings** and **Machine and resin** share one frame: a list 260 wide on `base` down the
left, beside it a page header (title 17, a muted second name, and tabs or actions at the
right), the page, and a bar 56 tall on `base` along its foot with the row's actions left
and the one that applies right.

- *Settings* lists, under *Find a setting* (`⌘F`, narrowing the pages by name and by the
  settings they hold), Tool profiles (Supports, Hollow, Drain, Recipes, each with its count),
  Application (Appearance, General, Updates, an accent `1` while a release waits) and Help
  (Keyboard shortcuts ↗) under 10.5 capital group names, and Machine and resin ↗ at the foot.
- *Appearance*: Theme as three `theme_card`s, System (both halves, *Now light* or *Now
  dark*), Light, Dark, the hint under them; Language; Motion. *General*: Start, Viewport,
  Sliced files, File name and Profiles, each folder a read-only path box (`~` for home) with its button
  beside it and a hint under; File name an editable box in code type, the reset arrow beside
  it once it differs from `{model}`, *Now saved as …* under it, an unknown `{tag}` named
  under that, the placeholders behind the question mark. Browser: Start, Viewport, File name. *Updates*, its title followed by *This is 0.6.0* in `text_low`:
  Release found, a `boxed` box with the New pill, the version, three changes, *And N more*,
  then Download and install (accent), Every change (opens the sheet), Skip this version;
  Looking, the daily switch, *Last looked …* with Check now at its right.
- A set of profiles is a `table`: words left, figures right (`text_mid`), group rows in
  `text_low`, the picked row washed with a 2-point accent mark at its left edge and its
  figures in `text_high`. The bar: Edit, Duplicate, Export, Delete (`confirm_button`).
- *Hollow* and *Drain*: the table alone, a double click turning a name into a `sunken` box
  over 60 % of its column (Enter or a click elsewhere renames), and under it at the right
  *Open the Hollow tool* ↗ (or Drain). The bar: Export, Delete.
- *Recipes*: the `table` of every recipe, the built-in ones first — name, *Type* (Built-in,
  Yours, or Has errors for one of yours that no longer reads or passes its check, why it
  does not as a `hint` under the table once picked) and the steps it takes — Import and
  *New recipe*, which keeps one of no steps and opens it, in the header. The bar: Edit (a double click too), Duplicate, Export, Delete (`confirm_button`,
  off with why for a built-in one). The form, under the trail *Recipes › Busts* (*Built-in*
  after a built-in one's name): a built-in one is read under a `notice` saying to duplicate
  it, every field off but its steps' folds. Blocks: *Recipe* (Name, the variables as a code-face name box, its
  figure and a bin a row, *Add a variable*), then the steps as a `chain_step` each on a rail:
  the number in a 22 disc on the rail, the grip, the kind's glyph, *Orientation*, and while
  folded, in `text_low` after it, *When …*, *Skipped if it fails*, *2 settings changed*, *1 rule*
  (*Switched off* alone for a step off; in `danger`, the disc ringed so, while it does not
  read); at the right its question mark — what that kind of step does, how a step runs and
  how a condition is written — its switch and the caret, and before the question mark move
up and move down, each off at its end. A press on the heading unfolds it,
  every step folded at first and each fold kept; one that does not read stands open. Open,
  the heading carries the bin before move up, the disc, filled over the rail so the rail stops at its ring,
  washed in `accent_wash` and ringed in `accent`; under it
  *When* as a `condition_row` (*Always* while empty), *On failure* Stop / Skip, its settings by kind — a setting left out its
  name and *Default* with a + to set it; one set its field (`number_row` with the unit its key
  names, `switch`, a `dropdown` of its words by what each means, the file's word at the row's right, `text_row`, points as rows of three fields and
  a bin) with the reset arrow back to the default and, for a number or a switch, the
  expression glyph trading it for a `condition_row` —, a gate's *Stop if* and *Message*, a
  pause's *Message*, a check's *Add supports to islands found* switch with *While* and *Max attempts*
  `nested` under it, then *Rules*: each rule `nested`, its *When* and settings and *Remove
  the rule*, and *Add a rule*; why the recipe does not run as a `field_error` at the top of
  the step it names, else a `danger` `notice` over the steps. A step dragged by its grip
  stands at 35 % while a card of its glyph and name follows the pointer, the page scrolling
  near its edges, and the seam it would land at is an accent line from the rail with a dot
  on it. Before, between and after the steps a `chain_seam`, 30 tall, a + on the rail, its
  list every kind, the one chosen added there and open; none while a step is dragged.
  *The recipe has no steps yet.* as a `hint` over the lone + while there are none. Every
  condition's list offers, under *The model*, *After a check*, *Your variables* and
  *Functions*, each name by what it means with its unit, the name at the right. Save at the bar's right is the accent only while there are unsaved edits
  that pass the check, its why on hover otherwise; leaving with unsaved edits asks as a
  profile's form does.
- A profile's form is blocks one under another. Its header: the way back, the trail
  *Supports › Heavy* (an automatic profile's kind, *Automatic sizing*, after the name),
  Reset to defaults at the right; Save at the bar's right, in the accent while anything is
  unsaved, Duplicate and Export greyed until then. Leaving with unsaved edits asks *Save the
  changes to Heavy?* over Save (accent), Discard and Cancel. Under it a 52 band of
  `section_tab`s, one a block, *OFF* after a block switched off; a press scrolls to the block
  and the one at the top of the view is lit. A support profile's blocks run Profile, Top,
  Pillar, Bottom, Small pillar, Branching, Grid, Raft, Cross bracing, Automatic, an automatic
  profile's Placing supports (its island areas under it), Sizes, Steps, Steadying; a switched
  block carries its switch beside its title and, while off, one line of what stands instead.
  Cross bracing: its presets, *Fine-tune* trading them for every number, then the buttress.
  An automatic profile's name is shown, not typed. A 180 title column, the title wrapping in it, beside at most 460 of fields, a hairline under each. A text row's name stands in a 150 column so
  every box starts on one line; every box ends on the block's right edge, and a figure
  worked out from them (*19.00 × 24.00 µm a pixel*) is a `field_note` flush with it. See Appearance and General above.
- *Machine and resin* lists the machines (search, then two-line rows with a network glyph and
  a tick on the one in use, Add a machine with import and a delete `bin_button` at the
  foot) and gives the picked one tabs: Resins (filter chips, Import, Add a resin, a table,
  *Take off this machine* as a `confirm_button`, *Use this resin*; a resin's form names its
  colour and opacity in one `colour_field`), Machine (blocks:
  Machine, Display with the pixel size right-aligned under its fields, Build volume) and
  Network (protocol, printers as rows with their live state, add by address, the ones found
  nearby as dashed rows with Add).
- With no machine yet the page is the library: a search over every machine, Import a
  profile and Custom printer at its right, and makers as 4-column tiles of glyph, name and
  count; a maker opened shows its models under a way back to every brand.

**Printer sheet**, 700 by 640: under its title the model, address and firmware; the plate's
picture beside the state (its word in the state's colour, the file, the percent at 22, a 6
point bar, layer and time left), Pause and Stop (`confirm_button`), light source and film
readings; the camera's address with copy and its switch; Recent prints three to a row, each
a 60 tall tile on `panel` of a 44 picture beside the file and its outcome in `ok`, `warn` or
`danger`.

**Find**, 500 wide, 70 from the stage's top over a 40 % scrim: a 48 tall field over a
hairline, results grouped under capital names as Tools, Actions, Models with their keys,
the lit row on `picked_wash`, Esc as a keycap at the field's right, and a strip over a
hairline at its foot with the keys to move and open (`key_row`).

**Update**, 640 wide, carries a release through: its head, then a `release_hero` (*Encrust
0.15.0* over *0.14.2 → 0.15.0*) with Release page ↗ at the
right; the changes scrolling, a `tag_label` over each group (New in `ok`, Better in the
accent, Fixed plain) and a `bullet` per change; the foot Skip this version left, Later and
Download and install right. Downloading, the foot turns to *Downloading · 12.4 of 38 MB*
and *about 6 s left* over the bar, the three `steps` (Download, Check the signature,
Install) and Cancel while it downloads; closing the sheet keeps it going, the pill in the
top bar showing the percent. A release found is said once a version by a notice at the
stage's foot (*Encrust 0.15.0 is out*, its first change and how many more, What is new,
the cross leaving the pill).

**Keyboard shortcuts** shares the Settings frame. The list: *Find a key* (`⌘F`, narrowing
the rows by name), All keys, then Keyboard (Tools, The plate, The view, The layers, Files,
The window) and Pointer (Mouse and trackpad), each with its count. The page: its name, *N
changed* in `text_low` after it, then `binding_row`s, under a `group_name` each group on
All keys. A key recording shows the chord pressed last, or the modifiers held, then Done
(accent once a chord was pressed). Done is answered under its row, if another action holds
the chord, by a warn `notice` (*⌘D already does Duplicate*) over Replace (accent) and Cancel. The bar:
Reset every key (`confirm_button`), and while a key records `Esc` Stop without keeping it.

## Start page

Under the top bar, padded 48 × 140 (the sides give way down to one tile's width): *Start a
plate* at 26 and the machine without its maker, resin and layer height under it; the drop zone, a 1.5 point dashed `line` box rounded 12 with a 52 square of
the drop glyph on `accent_wash`, its words, and Open a model (primary) and Open a project at
its right; then Recent / Test prints as a `Segmented` 220 wide and, on Recent with
anything in it, Clear the list at its right; then tiles of at least 180 on `panel` ringed in
`hairline` (`line` under the pointer), a 120 picture over the name, when and kind. A test
print kept in the repository, never in a browser, comes first and says Download for its
kind until fetched; while it comes, a `progress_bar` inset 12 runs along the picture's foot
and the kind is its percentage, or Downloading while the size is unknown; fetched, it says
STL and opens like the rest.

## A small window

At 900 by 560 the inspector is 272, the models and plate cards 232, the layer strip 64, the
tool row its glyphs alone, 36 tall, from 8 right of the inspector to 8 left of the models
card, the chip without the maker and the bar's right end glyphs only.
