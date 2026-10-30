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
│ │ foot        │       [notices]     [view tools] ┌plate┐  │ play   │                         │
│ └─────────────┘                                  └─────┘  └────────┘                         │
```

| Part | Where | Size |
|---|---|---|
| Top bar | the window's top, its own title bar (ADR 0104) | 44 tall |
| Inspector | the stage's left edge, from its top | 316 until dragged; as tall as its content, at most the stage |
| Tool row | over the stage, from 8 right of the inspector to 8 left of the models card | that width, 58 tall |
| Models card | 8 left of the layer strip, from the stage's top | 272 until dragged |
| Plate card | under the models card, standing on the stage's foot | 272, grows upward |
| Layer strip | the stage's right edge, its full height | 64 |
| Hint bar | centred under the tool row, 8 below it | as wide as what it says |
| View cube | 8 left of the models card, 8 under the tool row | 64 square |
| View tools | 8 left of the models card, 16 above the stage's foot | a column of 26 squares |
| Notices | centred between the inspector and the view tools, 16 above the foot | at most 440 wide each, 6 apart, newest nearest the foot |

Everything over the stage is a `card` on `CARD_SHADOW`; nothing over the stage carries a
stripe or a border of its own.

## Top bar

Left to right: on macOS 72 for the system's buttons, File, Edit, View as quiet text
buttons; in the centre the printer ring (while a printer prints; idle ones are reached from the plate card) and the chip; at the right
the Alpha pill, Find (200 wide, `⌘K` at its right), the key sheet and the gear as 30
squares.

The chip is three parts in one bordered box 32 tall: printer glyph and machine with a caret,
flask glyph and resin with a caret, sliders glyph and `layer height · exposure`. Each part
is its own button and lights in `accent_wash` while what it opens is open. Under 1100 the
chip drops the maker; under 720 the right end is glyphs only.

The machine and resin parts each open a chooser hanging 6 under the chip from its part's
left edge, 300 wide, on `raised` with a window's shadow, a capital group name over its rows:

- *Your machines*: a 44 two-line row a machine — glyph, name over maker and output format in
  `text_low`, an `ok` dot when a printer it lists answers on the network (`text_low` when none does,
  none when it lists none), the one in hand washed and ticked; under a rule, *Add a machine*
  (the library) and *Machine and resin…*.
- *On the machine's name*: a row a resin on that machine — its colour as a 10 point dot,
  name over type and layer height, its exposure at the right in `text_mid`, the one in hand
  washed and ticked; under a rule, *Edit this resin…* and *Add a resin* (a new resin, open
  in its form).

Choosing a row takes it in hand and closes the chooser, as Esc or a click elsewhere does; a
footer row opens Machine and resin on that machine's page or that resin's form.

The ring is 32 across, filled clockwise with the current print's fraction in the colour of
what the printer is doing, the percent in its middle and a count badge when more than one
printer answers.

## Tool row

Nine tools in three groups split by a vertical rule: Select, Measure | Repair, Hollow, Drain,
Cut, Relief | Supports, Check. Each is a `rail_button`: glyph 19 over its name in 10.5, 50
tall, the row's width shared equally, 2 apart. The tool in hand is washed in
`accent_wash` with its glyph and name in `accent_soft`; a tool that needs attention carries
a 7 point `warn` dot at its top right. While the pointer is on the row each tool shows its
key's digit, 1 to 9, at its top left in `text_low`. Narrower than 9 × 56, the names go and
each tool is a 36 tall square of its glyph.

## Inspector

One tool's card, in this order; any part a tool lacks is not drawn.

| Part | Content | Size |
|---|---|---|
| Heading | glyph in `accent_soft`, name, `help_mark`; at the right either the modes as `icon_toggle`s in one sunken group, or one fact in `text_mid` cut with an ellipsis | 48 |
| Preset | `picker` showing the preset (and `· edited` in `warn` once a value differs), Save and Delete as 32 squares | 32, padded 10 × 12 |
| Sections | `section`s, ordered what it acts on → how → rarely changed | heading 36, body padded 0 12 12, rows 6 apart |
| Foot | on `base`: one `primary_button`, at most one companion; the reason it cannot run under it | primary 40, padded 10 × 12 |

The body scrolls; the heading, preset and foot do not. What each tool holds:

| Tool | Heading right | Preset | Sections | Foot |
|---|---|---|---|---|
| Select | the picked model's name | — | Global / Local; Move (X Y Z, reset, drop to the plate); Rotate (X Y Z with −45° +45°, reset); Scale (Size and Ratio columns, Keep proportions, Fit the volume, Imported size); Place (Lay on face, Centre, Mirror, Mirror a copy, Lift off the plate); Auto-orient (Auto / Supports / Peel / Time, Auto-orient) | — |
| Measure | modes Points, Planes | — | Snap to (Corner / Edge / Free, Show dX dY dZ); Reading | — |
| Repair | modes Whole model, Paint | — | Found (`issue_row`s with counts, readings, look again); Method; Settings; Shells | Repair |
| Hollow | the wall | yes | Shell (Inside / Outside / Bottom open, Wall, Precision); Infill (switch in heading); Keep solid (Blocker size; a click on the model places one); Already done | Hollow |
| Drain | whether checked | yes | Trapped resin (a `notice`-style verdict); Opening (Hole / Channel, size, depth, taper, Round / Polygon); Plugs | Check drainage |
| Cut | the plane and its height | — | Plane (X / Y / Z, Height); Afterwards (Keep both / Lower / Upper, Lay the halves flat, Join the halves → `nested` joint kind and its rows); Parts | Cut, companion Split |
| Relief | modes Texture, Text, Combine | — | per mode: Where, Image, Depth, Leave flat — or Text, Lettering | Press it in, or the reason |
| Supports | modes Pick, Place, Paint where, Shape | the group picker | Picked; Where — or in Shape, Part with its diagram | Grow the supports, companion bin (every support) |
| Check | the number of issues | — | Found (`issue_row`s: kind, layer, count, the numbers on hover; previous / next as arrows in its heading; the all-clear verdict when there is nothing); Islands (Support them · Remove them side by side) and Cups (Drill them); Peel; Estimate (Time, Resin in ml · g, Price) | — (the check runs by itself) |
| Print settings | `layer height · exposure` | — | Layers; Exposure; By height; Edge smoothing with its state at the right; Motion and waits | Use the profiles' settings |

Print settings is opened from the chip, not the row; the row lights no tool meanwhile.

### The support group picker

The preset `picker` shows the group in hand: its colour dot, its name, `· edited`, its
support count, caret. Open, it is a list over the sections, as wide as the card less 12
each side, on `raised` with a window's shadow:

1. **On the plate** — one row per group holding supports: colour dot, name, count; on hover
   (always while hidden) hide and a `bin_button`. The group in hand is washed and ticked.
   Auto has no dot of its own; the shapes it built stand set in under it with theirs.
2. **Profiles** — the profiles holding nothing, in `text_mid`, no count.
3. A foot row, *Edit the profiles*, opening Settings → Supports.

Choosing a row takes its profile in hand and closes the list. The preset's bin removes the
group in hand with its supports; the foot's bin every support; both ask twice.

## Models card

| Part | Content | Size |
|---|---|---|
| Plates | numbered 26 squares, the current washed in `accent_wash`; + | row 42 |
| Heading | *Models*, `· project name` in `text_low`, `n picked` in `accent_soft`, + | 36 |
| Rows | eye or eye-slash, name cut with an ellipsis, a state tag (`Broken` in `danger_wash`) | 32; picked washed in `picked_wash`, no stripe |
| Actions | Duplicate, Array, Arrange, Remove as 26 squares, spread evenly | 34, hairline over it |

## Plate card

| Part | Content | Size |
|---|---|---|
| Heading | *Plate n*, the resin's name at the right in `text_low` | 36 |
| Figures | `figure_grid`: Time (`≈ 17 m 26 s`), Resin (ml), Weight (g), Price (the currency; `—` when the resin has none) | two rows of 46 |
| Details | `details_fold`: a table of Model / Supports for resin, weight and price, then Vertices and Triangles as `readings` | 26 closed |
| Verdict | a `warn` verdict when a model on the plate is broken | as tall as its text |
| Foot | the printer row, then the action row | on `base` |

The printer row is 34 tall on `raised`: printer glyph, name, a status dot and word (`ok`
Ready, `accent` breathing Printing n %, `text_low` Not answering), caret, and the open-sheet
square beside it. A profile listing no printer has no row.

| State | Action row |
|---|---|
| No printer, or the profile sends on a stick | Slice to .goo as the primary |
| A printer ready | Slice and send primary, Slice to .goo as the 40 square beside it |
| Choosing | the printer row opens a list of printers, each with its state, and *Scan again* last |
| Sending | the slice then the upload named over its bar, its percent under it, Cancel under that |
| The file landed | Start print primary, the 40 square beside it |
| The printer busy | Slice to .goo primary; under it *Bench A is printing a.goo; the file is saved instead.* |
| Nothing to slice | Slice to .goo disabled, and why under it |

A broken model's verdict is a `notice`: *1 model is broken*, *What is sliced from it may
not print.* In Preview the heading is *Layer*, the figures are Exposure, Block (bottom,
transition, body), Cured area and Volume of the layer shown, and there is no Details.

## Layer strip

Top to bottom, centred in its 64: the three views — model, model beside layer, layer — as
`icon_toggle`s in one sunken group, stacked; the layer number in a sunken box 20 tall in
`accent_soft` that a click opens for typing; `of n` and the layer's height in mm under it; layer up; the track;
layer down; play. The track is a 3 point rule, filled in `accent` between its two thumbs —
26 × 12, ringed in `accent`, the upper one the cut, the lower the first layer shown. Behind
the rule, every 4 points up it, the widest cured area there is a dashed `text_low` line
centred on it, at most 44 across, so the supports' base reads wide and the model narrow.
Across the rule a bar marks each risk the check found, in the colour the Check tool gives
it: `danger` for one that will fail (an island, a part left floating), `warn` for one that
may (a thin section, a hard peel, a lever, a suction cup), 22 × 3; the first of a run of
one colour, none within 12 points under it, 32 × 5. Scale ticks stand at the right edge in `line`. Nothing else is drawn on the
track. With no stack it reads *No layers*.

## Over the stage

- **Hint bar:** a pill on `base` at 88 % with a hairline, 12 point text: the key in
  `text_high`, what it does in `text_mid`, pairs split by a 12 point rule. Drawn only while
  the tool in hand has something the pointer does in the viewport (Measure, Drain, Relief,
  Supports' Pick and Place, Cut's plane).
- **View tools:** Frame the view, Plate grid, See through, Perspective, as 26 squares in one
  card; the on ones washed.
- **Notices:** each a card 40 tall: glyph in its colour (`warn` warning, `danger`
  warning-octagon, a turning circle-notch for a job), the words, at most one action as a
  small button washed in the accent, and a close square. A job carries its `progress_bar`
  and percent and a quiet Cancel instead of the close.
  No printer or no resin is a `danger` notice with the reason on a small line under it.
- **Empty plate:** nothing over the stage; the inspector says *Nothing on the plate* and
  *Drop a model into the window* under a 30 point cube-focus glyph, with Open a model as an
  inline button.

## Preview

The stage splits at its middle (ADR 0103): the model cut at the layer on the left, the mask
on black on the right, a pixel grid under it. Over the mask's top left, tags for the layer
and its height, the panel's pixels, and any issue on this layer in `danger_wash`; at its
bottom right, zoom out, the zoom in percent, zoom in and *show the whole mask* in one card.
The models card gives way to the plate card at the stage's top.

## Windows over the plate

A window is on `panel` with a 10 radius and a `line` ring, over a scrim of the stage at
62 %, at most 1080 by 720 and 24 from the window's edge; its title row is 48 — glyph, name,
close.

**Settings** and **Machine and resin** share one frame: a list 260 wide on `base` down the
left, beside it a page header (title 17, a muted second name, and tabs or actions at the
right), the page, and a bar 56 tall on `base` along its foot with the row's actions left
and the one that applies right.

- *Settings* lists Tool profiles (Supports, Hollow, Drain) and Window (General, Updates,
  Shortcuts) under 10.5 capital group names, and Machine and resin at the list's foot.
- A set of profiles is a `table`: words left, figures right, group rows in `text_low`, the
  picked row washed. The bar: Edit, Duplicate, Export, Delete (`confirm_button`).
- A profile's form is blocks one under another, its header the way back, the profile's
  name and its kind: a 180 title column beside at most 460 of fields, a hairline under each; Reset to defaults in the header. A text row's name stands in a 150 column so
  every box starts on one line; every box ends on the block's right edge, and a figure
  worked out from them (*19.00 × 24.00 µm a pixel*) is a `field_note` flush with it. General holds Language, Opening, Motion
  (*Reduce motion*), Sliced files, the Prusa login and the bug report.
- *Machine and resin* lists the machines (search, then two-line rows with a network glyph and
  a tick on the one in use, Add a machine with import and a delete `bin_button` at the
  foot) and gives the picked one tabs: Resins (filter chips, Import, Add a resin, a table,
  *Take off this machine* as a `confirm_button`, *Use this resin*), Machine (blocks:
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

**Keyboard shortcuts**, 800 wide, *Find a key* in its title row narrowing the rows by name:
the keys in two columns of groups under 10.5 capital
names — Tools, The plate, then The window, The layers, Files — each a row of name and keys.

## Start page

Under the top bar, padded 48 × 140 (the sides give way down to one tile's width): *Start a
plate* at 26 and the machine without its maker, resin and layer height under it; the drop zone, a 1.5 point dashed `line` box rounded 12 with a 52 square of
the drop glyph on `accent_wash`, its words, and Open a model (primary) and Open a project at
its right; then a shelf heading, *Recent*, with Recent / Test prints as a `Segmented` 220 wide and, on Recent with
anything in it, Clear the list at its right; then tiles of at least 180 on `panel` ringed in
`hairline` (`line` under the pointer), a 120 picture over the name, when and kind.

## A small window

At 900 by 560 the inspector is 272, the models and plate cards 232, the layer strip 64, the
tool row its glyphs alone, 36 tall, from 8 right of the inspector to 8 left of the models
card, the chip without the maker and the bar's right end glyphs only.
