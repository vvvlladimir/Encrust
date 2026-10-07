# The profile catalogue

How a printer, a resin and a support profile reach the slicer: where the files live, how an id resolves, and
how one resin becomes the right numbers for five different machines.

Why it is built this way is in `docs/decisions/0049`, `0050`, `0051` and `0119`.

## What is on disk

```
assets/profiles/
├── printers/          one file per machine, id = file stem
│   ├── elegoo-mars-4-ultra.toml
│   └── ...
└── supports/          one file per support profile, id = file stem
```

There is no `resins/`: an exposure is measured on the machine in the room, so the
catalogue ships none and the first resin is the user's own (ADR 0196).

Every file under `printers/` and `supports/` is embedded into `printer-profiles` by its
build script, which emits an `include_str!` table sorted by id; a kind with no directory
ships an empty table. Adding a machine is adding
a file; nothing else has to be edited. Most of `printers/` is written by the generator at
the end of this page; a file without its marker line was written by hand and the generator
leaves it alone.

The user's own directory has the same shape:

```
<config>/Encrust/profiles/printers/<id>.toml
<config>/Encrust/profiles/resins/<id>.toml
<config>/Encrust/profiles/supports/<id>.toml
```

`<config>` is the platform configuration directory, or whatever `ENCRUST_PROFILE_DIR`
names. A file there whose stem matches a shipped id replaces it; a new stem adds a
profile. Nothing is merged inside a file: the user's file is the whole profile.

That directory is also what the user **has**: the window lists a profile only once it is
there, and the shipped catalogue is the library it is installed from, so a first run has
no printer and no resin (ADR 0158). The CLI resolves any catalogue id, installed or not.

A copy under a shipped id is a copy: a later release correcting the shipped numbers does
not reach it. The printer form says so and offers the shipped profile back (ADR 0196).

## Resolving

```
Catalogue::bundled()   the embedded table, parsed
Catalogue::load()      that, with the user directory laid over it by id
catalogue.printer(id)  -> Entry { id, profile, source }
catalogue.resin(id)    -> Entry { id, profile, source }
catalogue.resin_for(resin_id, printer_id) -> MaterialProfile
```

`Source` is `Bundled` or `User(path)`, which is what the picker's `[yours]` mark and the
CLI's `[user]` column come from.

## One resin, five machines

A resin file is the resin, then one table per machine:

```toml
name = "Standard grey"
exposure_s = 2.6
lift_speed_mm_min = 65.0

[printers.elegoo-mars-3-pro]
exposure_s = 3.2
bottom_exposure_s = 38.0

[printers.elegoo-saturn-4-ultra]
exposure_s = 2.3
lift_speed_mm_min = 120.0
```

`PrinterTuning` is the set of settings a machine may change. Every field is optional and
unknown keys are refused, so a misspelt key fails to load rather than doing nothing:

| Group | Fields |
|---|---|
| Light | `exposure_s`, `bottom_exposure_s`, `bottom_layers`, `transition_layers`, `light_off_delay_s`, `light_pwm`, `bottom_light_pwm` |
| Motion | `lift_distance_mm`, `lift_speed_mm_min`, `retract_distance_mm`, `retract_speed_mm_min`, `bottom_lift_distance_mm`, `bottom_lift_speed_mm_min`, `bottom_retract_speed_mm_min` |
| Waits | `waits`: light-off delay or rests before lift, after lift and after retract |
| Geometry | `layer_height_mm`, because an exposure is only valid at the height it was measured at |

`name`, `density_g_cm3` and `[details]` — type, colour, price, currency and whether the
price is per kilogram or per litre — are not tunable: none changes with the machine.
`last_printer` names the machine the resin was last saved for.

`for_printer(id)` clones the resin, applies that machine's table and drops the tuning map,
so what the slicer, the window and the writers see is a plain `MaterialProfile`.
`starting_point(id)` is what every caller asks for: that machine's table, or for a machine
with none, `last_printer`'s, or the resin's own numbers. `is_tuned_for(id)` is what the
panel and the CLI use to tell a resin set up on the machine from one that is not. Validation runs over every resolved variant at load time, so a zero
exposure hidden in one machine's table is caught when the file is read, naming the machine.

## What each binary does with it

`encrust slice --printer <id> --resin <id>` resolves both through the catalogue; `--profile` and
`--material` take paths and win over the ids. `encrust profiles list` prints the catalogue
and exits. A machine named without a resin is refused: nothing ships an exposure, so there
is nothing to fall back on (ADR 0196). A report over geometry — `inspect` — still runs.

The window keeps the catalogue in `Slicing`, along with the resin as loaded and the resin
as resolved. Picking a printer sets the plate, the container it writes, the machine on the
network it sends to, and retunes the resin in hand; picking a resin retunes it for the
printer in hand. Both menus end in the file dialog
the window had before, which sets no id and therefore tunes nothing.

## Editing in the window

The gear at the right of the title strip opens the Settings screen, which replaces the
plate; Esc leaves it. Its profile pages, Printers and Supports, are each a list beside a form, and every
edit is written to the user's directory as soon as the value settles — no Save. ADR 0120.
The third page, Updates, holds no profile: see `docs/design/updates.md`.

- **Printers** lists the machines the user has installed, by brand, over a search line.
  The form carries the panel, the volume, the container
  `output` names, how the machine is reached, and the firmware table, and says when it
  differs from the machine of the same id this build ships. The one open shows
  the resins set up on it, each with
  buttons to duplicate it or take it off, and **Add resin**: a new one, or one from the
  pool of every resin some printer has, which opens on `starting_point`.
- The `+` over the list opens the **library** in the form's place: a search over every
  machine there is, a card per brand, and that brand's models behind it. Picking one
  copies the shipped profile into the user's directory and closes the library onto it;
  **Custom printer** does the same with an empty profile. Removing a machine deletes that
  copy — asked about first — and a shipped one is back in the library to install again.
  The window moves to the machine under it in the list, or to none when it was the last.
- **Add resin** offers every resin on another printer, and **New resin** makes one; a
  printer with none and an empty pool offers that one button instead of the menu. A resin
  taken off a printer waits in the pool, and the pool is where it is deleted for good —
  except one nobody typed into, which goes with the printer it was made on (ADR 0197).
  Every deletion is asked about first, because the only one this screen takes back is a
  resin added again from the pool (ADR 0196).
- A resin's form edits it **on that printer**: numbers go into its `[printers.<id>]`
  table and make it `last_printer`; name, type, colour, density and price go on the resin.
  Renaming one other printers share splits it off as this printer's own file.
- Ids are made from the name when the profile is made (`unique_id`) and never follow it.

Saving the machine in use stands the plate back under it when the panel, the volume or
the container changed. Saving the resin in use re-resolves it for that machine.

Nothing is ever written into `assets/profiles`: that is the shipped catalogue, and the
user's directory is what wins over it.

## How a machine is reached

`connection` says what a machine takes a file over: `none` for a stick, `sdcp` for an
Elegoo board, `prusa-link` for an Original Prusa. The address and the credentials are not
in the profile — they are the window's own settings — but which machine on the network
this profile sends to is remembered against it, so one printer is set up once and the
Slice button goes there from then on (ADR 0156).

## What the file calls the machine

A sliced file names the machine it is for, and the firmware matches that string. It is
`name` unless the profile states `machine_name`, which is what the vendor's own slicer
writes: `Saturn 4 Ultra` in the picker, `ELEGOO Saturn 4 Ultra` in the file (ADR 0140).

## What the firmware obeys

A printer file's `[firmware]` table says what the machine honours beyond the header:

| Key | Default | Meaning |
|---|---|---|
| `per_layer_settings` | `true` | Reads the per-layer tables. Chitu 4.3.9 and later do. Off clears `.goo`'s advance mode (ADR 0141). |
| `variable_layer_height` | `false` | Moves the plate to each layer's own Z rather than stepping by the header's height. |

Neither can be read off the file format — machines of the same manufacturer and format
differ, and a firmware update has taken the second away before. No shipped file claims
`variable_layer_height`: it is the user's own tested claim, and a job that needs it is
refused without it (ADR 0091). The Saturn 4 Ultra is the one shipped profile that clears
`per_layer_settings`: it separates a layer by tilting its vat, and obeying the per-layer
lift stalls the plate (ADR 0142).

## Exposure follows the layer height

`layer_height_mm` in a resin file is not a default, it is the thickness the exposure was
measured at. Slicing at anything else carries the exposure along the resin's Jacobs working
curve, `E(L) = Ec exp(L / Dp)`: its own `penetration_depth_mm` where it states one, and
`ASSUMED_PENETRATION_DEPTH_MM` of 0.10 mm where it does not (ADR 0143). Halving the layer
height therefore takes about four fifths of the exposure, not half. The bottom block is
exempt: its long exposure is for sticking to the plate.

In the window this happens in sight: changing the layer height carries the normal exposure
and the bands along at once, marks them, and offers the old value back (ADR 0128). The
resin editor does the same when the height a profile is measured at moves.

## What the print comes out as

A resin also carries a `[compensation]` table: shrinkage per axis, the tolerance offsets
on holes and outer walls, and the seconds a layer costs beyond what the settings account
for. It is tuned per machine like everything else here. What each field means and how to
measure it: `docs/design/compensation.md` (ADR 0144).

## Where the numbers come from

Every shipped file is transcribed from a vendor specification or a public machine list and
carries an `UNVERIFIED` header. They are a starting point. An exposure test on the
actual machine is what replaces them, and a corrected file in the user directory is how it
is kept.

## Growing the catalogue

```sh
cargo xtask gen-profiles --source <dir of .ini profiles> \
  [--cross-check <machine list>] [--out assets/profiles/printers] [--dry-run]
```

The source is a public set of printer profiles, one `.ini` per machine — about 150 machines
across 21 manufacturers. The machine list published beside them is generated from the same
files, so `--cross-check` holds a transcription against it rather than taking anything from
it. It is run by hand against a local checkout, never from a `build.rs`: the build has no
network, and a catalogue that changes under a rebuild is not reviewable (ADR 0165).

An `.ini` carries everything a printer file of ours needs but the container:

| `.ini` | our file |
|---|---|
| `display_pixels_x`, `display_pixels_y` | `[display] width_px`, `height_px` |
| `display_width`, `display_height` | `[display] width_mm`, `height_mm`, and `[build_volume] x`, `y` |
| `max_print_height` | `[build_volume] z` |
| `display_mirror_x`, `display_mirror_y` | `mirror_x`, `mirror_y` |
| `printer_notes: FILEFORMAT_<ext>`, `FILEVERSION_<n>` | `output` |

The brand is the first word of the file name that names one of the twenty the generator
knows, and the model is the rest; the id is the two in kebab case, which is the file stem.
`connection` is not in the source and is a table in the generator, holding only the
generations `docs/formats/sdcp.md` and `docs/formats/prusalink.md` name. `machine_name` is
only known from a file the vendor's own slicer wrote, so it is left out until one is read.
There are no exposures in the source — the `CUSTOM_VALUES` block carries lift, speed and
PWM, not a measured cure — and none is invented here.

Five rules the generator keeps:

1. **Facts, not files.** A resolution, a panel size and a travel are measurements, and
   they are transcribed into our own schema. The `.ini` files are not vendored, and the
   judgements made about them are recorded in the generator. Our own licence binds code,
   not measurements.
2. **Ship nothing we cannot write.** A machine whose container has no writer is reported
   with the step that brings its codec, and no file is written for it. The one exception
   is a `.ctb` below version 4: version 4 is the oldest we write, so the profile points at
   it, says so in its header and clears `firmware.per_layer_settings`.
3. **Own only what you generated.** A catalogue file carrying the generator's marker line
   is rewritten; one without it was written by hand and is kept, so a profile corrected
   against a real machine survives every later run. A generated file the source no longer
   names is reported, not deleted.
4. **Every file stays `UNVERIFIED`** until somebody prints the exposure test. The user
   directory is where a corrected machine lives.
5. **Tests over verification.** Nobody owns 150 machines. Every shipped profile has to
   load and resolve, its pixel pitch has to be plausible, the extension its container
   writes has to route back to the same writer, and its panel has to match the published
   machine list — which is what catches a transcription slip. Where the list is the stale
   one, that is a line in the generator naming what settles it.

A run prints what it wrote, what is waiting on a later step, what it kept, where it took
the source over the list, and what it refused; it fails on a refusal.
