# The `encrust` command line

Every subcommand, with the shape of an invocation that uses it. `encrust <command> --help` is
authoritative; this file is the worked examples and the contract a script can rely on. The
window reaches the same code through `core-engine`. In a checkout, `encrust` below is
`cargo run --release -p encrust-cli --bin encrust --`.

`--profile` and `--material` take a TOML path and win over the catalogue ids. The
catalogue ships machines and support profiles but **no resin**: an exposure is measured on
your own machine, so the first resin is yours to make, in the window or as a TOML file
(ADR 0196). `my-grey` below is one of yours; `profiles list` says what you have. A machine
named without a resin is refused rather than given stock numbers.
`--slice-window` and `--raster-window` trade memory for time on a dense stack (ADR 0066).
The dev profile is optimised (ADR 0037); every quoted timing is a release timing.

A model is welded as it is read, vertices nearer than `--weld-tolerance` mm merged;
`--no-validate` skips the topology check and the orientation fix; `--repair` stitches seams,
drops specks, patches holes and drops doubled faces as the window's Repair does, keeping the
model as it came if that would leave more wrong (ADR 0349). `--repair=close-holes`,
`=close-small-holes` or `=turn` runs part of it (ADR 0351), `=wrap` wraps every shell in one
outer surface by the exact union of its shells and `=deep` repairs, wraps and repairs
again, leaving the repair alone where the shells do not union exactly (ADR 0352, 0357); a
cavity wound inward is kept, a part wholly inside another is lost in it (ADR 0358).
`inspect --intersections` counts the pairs of faces that cross (ADR 0353).
`--seam-tolerance MM` (0.01) is
how far a vertex may stand off an open edge to be stitched in (ADR 0350), `--smallest-shell
MM2` (1) the least area an open shell keeps. By hand, as the window's Faces mode and Shells
list do (ADR 0356): `--flip-faces 3,4` turns faces and `--drop-faces` drops them, numbered
from 0; `--drop-shells 2` drops shells numbered from 1 as `inspect --shells` lists them;
`--bridge A-B:C-D` lays faces between two open edges given by their vertices. Every number
is of the model as import leaves it; turns and bridges go first, drops last, before
`--repair`. `--samples-per-layer N` cuts N planes inside each layer,
so a feature thinner than a layer is kept. A mask is
shaded as the printer profile says unless `--no-anti-alias` lights each pixel fully or not
at all, `--grey-levels N` rounds its edges to N greys, `--grey-floor V` sets the dimmest
grey written or `--blur PX` fades every edge.

```sh
encrust profiles list
encrust profiles show elegoo-mars-4-ultra -o my-printer.toml   # a profile to start from
encrust info plate.ctb                       # open a sliced file
encrust inspect model.stl --printer elegoo-mars-4-ultra
encrust inspect broken.stl --repair            # what closing its holes would do
encrust slice broken.stl --repair=close-small-holes -o out   # only the missing faces
encrust inspect parts.stl --repair=wrap --intersections   # one surface, crossing nothing
encrust inspect parts.stl --shells             # every shell, numbered from 1
encrust slice parts.stl --drop-shells 2,3 -o out   # without the two loose pieces
encrust slice model.stl -o out               # PNG stack

encrust slice model.stl \
  --printer elegoo-mars-4-ultra --resin my-grey --center -o model.goo

encrust slice model.stl \
  --printer elegoo-mars-3-pro --material my-grey.toml --center --ctb-version 5 -o model.ctb

encrust slice model.stl \
  --printer elegoo-mars-3-pro --resin my-grey --center -o model.cbddlp     # or .photon

encrust estimate model.stl \
  --printer elegoo-mars-4-ultra --resin my-grey   # time, resin, price; writes nothing
encrust convert plate.ctb --printer elegoo-mars-4-ultra -o plate.goo
encrust printer send model.goo 192.168.1.42 --start      # to an Elegoo board
```

## Output, exit codes and Ctrl-C

The global flags go before or after the subcommand. `--json` prints exactly one JSON
document to stdout, with `"schema": 1` beside the report's own fields; a failure prints
`{"schema": 1, "error": {"message", "chain", "cancelled"}}` there instead, and the text
still goes to stderr. Logs always go to stderr; `-q` keeps only errors, `-v` and `-vv` add
debug and trace, and `RUST_LOG` wins over all three. `slice` of one model, `inspect` and every
model of a batch share one report shape; `slice` of a plate prints one entry per model under
`models` beside the stack they make, and `estimate` adds `print` — layers, height, time,
resin, weight, cost and the risks — when there is a printer to draw the masks for.

Resin is counted from the masks and nowhere else — `cured volume` in the text, `cured` in
the JSON — so a run without a printer reports layers and no resin at all (ADR 0206). The
`volume` an `inspect` of a model with several shells prints is their sum, and says so where
they can overlap.

| Code | Meaning |
|---|---|
| 0 | Done |
| 1 | The run failed |
| 2 | The arguments were wrong |
| 3 | `--strict`, and a model has defects or does not fit; nothing is left on disk |
| 4 | A batch in which at least one model failed |
| 130 | Stopped by Ctrl-C |

`--strict` keeps nothing it would not print: the file or the PNG directory of a run that
ends in code 3 is taken off the disk again, so the exit code and what is there say the same
thing. The report still names every defect that led to it.

A number typed after a flag may be negative — `--rotate -90,0,0`, `--relief -0.5`,
`--drain-at -5,0,0` — and `--rotate=-90,0,0` is the same thing.

On a terminal `slice` draws a bar over the layers on stderr, and `batch` one over the
models; `--no-progress`, `--quiet` and `--json` turn it off. The first Ctrl-C stops the run
between layers, inside the cavity a `--hollow` run is building and between the models of a
batch, and removes the file it was writing; a second one exits at once. `inspect`, `info`
and `profiles` write at most one small file, so one Ctrl-C ends them.

The output extension picks the format (ADR 0047): `.goo`, `.prz`, `.ctb`, `.cbddlp`, `.photon`,
`.sl1`, `.sl1s`, `.zip`, `.crmaslice`, `.cxdlp`, `.svgx`, `.cws`, `.fdg`, one of the Anycubic extensions in
`docs/formats/anycubic.md` or `docs/formats/pwsz.md`, or a name with no extension for a directory of PNGs. A container
name is refused without `--printer` or `--profile`: the panel is what the masks are drawn on,
and a run that cannot write the file it was asked for fails rather than reports success.
`--ctb-version` is read only by a `.ctb` name, and a `.ctb` is written encrypted when the
printer profile says `output = "ctb-encrypted"` (ADR 0298). A `.cxdlp` name is written at version 3 unless
the printer profile names version 4, as the Halot Mage line does (ADR 0168).

Each container pays for grey differently. A `.cbddlp` carries it in eight one-bit passes, so
it is eight times the layer data of the others (ADR 0146). An Anycubic file carries four bits
of it in one pass, which costs nothing but loses an edge dimmer than 16 (ADR 0147). An `.sl1`
carries all eight bits and loses nothing, because its layers are PNGs, and so do a `.zip` and
a `.cws`. A `.cxdlp` carries eight bits at version 3 and seven at version 4. An `.svgx` carries
none at all: its layers are polygons, and a pixel is lit or dark at half coverage (ADR 0169).
Those two cost by the panel rather than by the model, so a run writing one for a panel larger
than any of its own machines have — a `.cbddlp` above 1440 x 2560 px, an `.svgx` above
3840 x 2400 — says so before it writes a layer.

An `.sl1` states one exposure and one layer height for the whole stack, so `--exposure-at`
and `--adaptive` are refused rather than flattened under an `.sl1` name (ADR 0148). A `.zip`
is the other archive and carries both, because every number it states is per layer in its
`run.gcode` (ADR 0167).

`info` opens any of those containers, whoever wrote it, prints what the file states, and
then decodes every layer to check that it does: a header can be read from a file no machine
would print. A file whose name says nothing is recognised by its own first bytes (ADR 0149).

Hollowing, with an infill lattice in the cavity. `--hollow-mode internal` grows the wall
inward, which is the default; `external` grows it outward and makes a mould of the model.
`--hollow-open-floor` opens a shell through the floor it stands on, so it drains with no
hole; a mould has no floor to open and refuses it. Otherwise nothing opens the cavity by
itself — `--drain-at` below is what lets the resin out:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --hollow 2 \
  --infill hive --infill-cell 6 --infill-density 0.3 --precision 0.5 -o model.goo
```

Every pattern takes `--infill-cell` (a hexagon's diameter, a square's side, a cube's edge)
and `--infill-density`, 0 to 1; the walls or struts and the crossing holes follow from them
(ADR 0320). Every number left out is the window's default, and one given without
`--infill` is refused.

Modelling, as the window's Relief and Cut tools do it, before the models are hollowed and
supported (ADR 0312). `--combine union|difference|intersection` makes every model given one,
a difference keeping the first; `--label TEXT --label-at X,Y,Z` writes on the surface nearest
that plate point, with `--label-height`, `--label-depth`, `--label-sunk`, `--label-follow`,
`--label-angle` and `--label-font`; `--cut x|y|z:MM` cuts at a plate coordinate, `--keep
both|lower|upper` picks the halves and `--joint dowels|pins|magnets|key` joins them at the
window's default sizes; `--split` breaks a model into its bodies, scraps under
`--split-smallest` joined or dropped, and `--part-off X,Y,Z` cuts off the part that point is
on. A half that no longer reaches the floor is laid on its cut face, and a cut needs
`--arrange`. A piece of several is not hollowed or supported, and a project refuses the
flags.

```sh
encrust slice base.stl lid.stl --printer elegoo-mars-4-ultra --resin my-grey \
  --combine union --label "No 7" --label-at 20,10,30 --label-sunk -o part.goo
encrust slice bust.stl --printer elegoo-mars-4-ultra --resin my-grey \
  --cut z:60 --joint dowels --arrange -o bust.goo
```

A hole into the surface nearest a point, cut with or without a cavity behind it, and the
check that says whether any resin is still stuck. `--check-drainage` asks for it alone. The
point is in plate millimetres from the plate's front left corner, after `--center` and the
rest of the placement; `encrust inspect model.stl --center --printer …` prints the bounds to
pick it out of:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --hollow 2 --drain 4 --drain-at 30,30,60 -o model.goo
```

A point off an edge or a corner of the model, rather than over a face, is refused: the hole
would be centred on the edge, half off the model, and reach no cavity. `--drain-depth` is how
far past the surface a hole reaches and `--drain-taper` its far end's share of its mouth, 1 a
cylinder. A hole is round unless `--drain-sides 3..12` cuts it as a polygon, its corners on the
diameter, turned `--drain-turn` degrees from the level; a `--channel` takes the same
outline, `--channel-size` millimetres across. Once the model is hollow a hole is carried on through the wall into the cavity;
`--drain-walled` drills it before hollowing instead, a recess at its depth with the wall
kept around it, which drains nothing (ADR 0346). `--drain-support-at X,Y,Z` drills into the
supports or raft nearest the point once they stand, sized as `--drain`, with no plug (ADR 0347).
`--drain-plugs` prints a plug for each hole, standing on the plate past the model's
right-hand side: what the hole took out of the wall, `--plug-gap` millimetres narrower a
side (0.15 by default, never less than 0), its outer face flush with the surface (ADR 0313).
A resin whose tolerance compensation is off takes its hole and outer offsets off that gap,
so the fit prints as asked. A run whose plugs would land off the plate or on another model
stops and says so; `--arrange` does not make room for them.

```sh
encrust slice figure.stl --printer elegoo-mars-4-ultra --hollow 2 \
  --drain 5 --drain-sides 6 --drain-at 30,30,0 --drain-plugs -o figure.goo
```

The model's own texture pressed in as relief, as deep as the plate millimetres given.
Negative sinks it in. Needs a model carrying UVs and an image beside it — or
`--relief-image`, a picture tiled over any model from three sides, `--relief-tile`
millimetres wide (12 by default) and turned `--relief-angle` degrees (ADR 0225). It presses
the whole model; a press into painted faces only is painted in the window (ADR 0318):

```sh
encrust slice textured.obj \
  --printer elegoo-mars-4-ultra --relief 0.4 --precision 0.6 -o model.goo

encrust slice part.stl \
  --printer elegoo-mars-4-ultra --relief 0.4 --relief-image knurl.png --relief-tile 6 \
  --relief-angle 45 -o part.goo
```

`--relief-invert` reads black as high, `--relief-symmetric` reads mid grey as the surface
and moves it half of `--relief` either way, and `--relief-smooth` blurs the texture by that
many of its pixels first. `--relief-hold-bottom` and `--relief-hold-top` leave the faces
within that many degrees of facing the plate, or straight up, as they are (ADR 0308):

```sh
encrust slice part.stl \
  --printer elegoo-mars-4-ultra --relief 0.4 --relief-image stone.png --relief-symmetric \
  --relief-smooth 1.5 --relief-hold-bottom 5 -o part.goo
```

Exposure of its own over a band of print height, repeatable. The bottom block keeps the
resin's ramp whatever a band says:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --exposure-at 4:6:9.5 -o model.goo
```

Thick layers on a vertical wall, thin on a shallow slope, against a cusp target. Needs a
printer profile whose `[firmware]` claims `variable_layer_height`, which no profile of the
catalogue claims — the plate has to step to each layer's own Z, and that is a property of a
machine's firmware rather than of its container. Run it against a profile of your own, and
the refusal comes before the stack is cut rather than when the file is written.
`--min-layer-height` defaults to half the layer height, because every thickness is a whole
number of it: a floor the layer height is not a multiple of lowers the thickest layer the
run can reach, and the run says so.

```sh
encrust slice model.stl \
  --profile my-printer.toml --adaptive --cusp 0.03 --min-layer-height 0.025 -o model.goo
```

Every island taken out of the file, and nothing that stood on one: what stood on it is
named as floating (ADR 0228). `--hanging-passes N` takes N layers more of what is left
hanging, one layer a pass, as the Check tool does:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --remove-islands --hanging-passes 2 -o model.goo
```

Turn the model the way it prints best, at its `--scale` and before anything else, reported as the
`--rotate` that repeats the turn — `--orient-goal` searches for `fewest-supports` (the
default), `least-peel`, `shortest-print` or `auto`, which balances the three, and
`--max-tilt DEG` keeps the turn within that many degrees of the way the file stands (ADR 0222) — and build supports to a profile of the catalogue —
`light`, `medium`, `heavy` or one saved in the window, as `profiles list` names them — or to
your own TOML with `--support-profile`; `--supports auto` sizes each support by what it holds
into the automatic profile's shapes, as the window's **Auto** does (ADR 0325), and
`--support-cavity` holds up the cavity `--hollow`, or a plate file's `hollow`, leaves as well
as the outside, as the window's **Support the cavity** does (ADR 0332). A plate file's `supports` takes `auto` too. With both, the search counts as overhang what that
profile's `max_overhang_deg` does, and 45 degrees without one. With a printer as well the
search's best few are printed to choose between, as the window's **Auto-orient** does
(ADR 0290). A turned model is stood on the plate over the middle it stood over in x and y.
A model the search sets on the plate gets supports only under what would hang there, and none
if nothing does; with supports and a raft it stands on the raft's top. A `--rotate` that tips
it off that face stands it on supports. For example:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --orient --orient-goal least-peel --max-tilt 30 \
  --center -o model.goo

encrust slice model.stl \
  --printer elegoo-mars-4-ultra --supports medium --center -o model.goo

encrust slice model.stl \
  --printer elegoo-mars-4-ultra --hollow 2 --supports auto --support-cavity -o model.goo
```

A model is stood off the plate by the profile's own `z_lift_mm`, and by its raft's
thickness on top when it has one, before anything is put under it, the way the window's **Lift off the plate** does it: a part sitting on the plate has no
room for a support. One already standing that high is left where it is, and a profile with
`z_lift_mm = 0` lifts nothing. The report says how far it moved.

## Plates: several models, a plate file, a project

Several models go on one plate. `--arrange` spreads them, biggest first; without it each
stands where its file put it. `--center` and `--drain-at` point at one model, so they are
refused for several:

```sh
encrust slice a.stl b.stl c.stl \
  --printer elegoo-mars-4-ultra --resin my-grey --arrange --supports light -o plate.goo
```

A plate file says the same per model. Paths are relative to it, every key is optional but
`path`, and a key it does not know is an error:

```toml
printer = "elegoo-mars-4-ultra"
resin = "my-grey"
layer_height_mm = 0.05
arrange = false                 # true spreads every model; it cannot be mixed with position

[[model]]
path = "a.stl"
rotate = [0, 0, 45]             # degrees around X, Y, Z
scale = 1.2                     # or [x, y, z]
position = [60, 40]             # where the middle of the footprint goes, plate mm
supports = "medium"             # any support profile `profiles list` names

[[model]]
path = "b.stl"
hollow = { wall_mm = 2.0, mode = "internal" }   # or "external" for a mould; open_floor = true opens a shell
```

A project saved by the window slices as it was saved: the file holds its shells and its
support trees, and nothing is built again (ADR 0191). It carries its own models, so a flag that shapes one — `--hollow`, `--supports`,
`--rotate`, `--blur` and the rest — is refused; `--printer`, `--resin` and `--layer-height`
replace the project's own, and `--slice-window` applies as to a model. The project names
its printer and resin by id only (ADR 0261): one that is not among your profiles has to be
named by a flag, `--profile` and `--material` included.

A project of several plates slices the plate it was saved on and says so; `--plate N`
picks another, counted from one as the plate bar shows them, for `slice` and `estimate`.
`slice --all-plates` writes every plate holding a model into a file of its own named after
it, `plate-small-parts.goo` beside `plate.goo`, as the window's Slice all plates does; its
`--json` is one document with a report per plate under `plates`:

```sh
encrust slice plate.toml -o plate.goo
encrust slice plate.encrust --resin my-grey -o plate.goo
encrust slice plate.encrust --plate 2 -o small.goo
encrust slice plate.encrust --all-plates -o plate.goo
```

A flag wins over the plate file or project, which wins over the profiles' own numbers.

## Estimate, info and convert

`estimate` takes what `slice` takes and writes nothing. It prints what the stack is and,
with a printer, what the print takes: time, resin by volume and weight, price where the
resin has one, and the layers likely to fail. `--strict` fails it on the same defects. The
`fits` line answers for the model where it stands, not only for its size: one small enough
to print but hanging off the plate is named as off it, because the panel clips whatever
hangs over and the resin figure then counts material no machine would cure.

`info --layer N` reports one layer of a sliced file, counted from one as a printer's screen
counts: where it tops out, how thick it is, how long it burns and how much of the panel it
lights. `--png out.png` writes that layer's mask beside the report.

Over the whole file, `info` names every run of layers exposed differently from the header —
a resin's transition ramp is one such run, an `--exposure-at` band another — and for a stack
that does not keep one height, its thinnest and thickest layer in place of the header's one
number.

`convert` writes a sliced file again in the container its output names, for the printer
given. The masks are copied, never resampled, so the printer's panel must be the size the
file was drawn for, in pixels and, where the file records it, in millimetres; the layer heights and every exposure are the file's own, and the resin
gives the lifts, waits and price. A ramp above the bottom block is read back as the resin
transition it is and stated again as one, so a machine that reads the header alone still
takes the file; any other run of exposures becomes a band of height, which such a machine
refuses. A stack of varying layer heights is refused for now, and
the new file's previews are blank (ADR 0180).

## Profiles, a config file, completions

`profiles list` prints the catalogue in three blocks — printers, resins and support
profiles — marking what came from your own folder, then the Hollow and Drain presets saved
in the window, if any; the resin block holds only what you made, since none is shipped; a
support profile saved in the window is in that third block and `--supports` takes its id.
`profiles show <id>` prints any of them as the TOML it is kept in, or writes it with `-o`;
edited, it is what `--profile`, `--material` and `--support-profile` take. `--kind` picks
one when two kinds share an id. `profiles import FILE --kind KIND` takes a file exported
from the window or by `show` into your own folder, under an id of its own and never over
one you have; `--printer ID` sets an imported resin up on that machine (ADR 0233):

```sh
encrust profiles import grey.toml --kind resin --printer elegoo-mars-4-ultra
```

A file in your folder that no longer reads is left out with a warning, and the shipped
profile of its id is used. `profiles reset-unreadable` throws away those copies of shipped
profiles; a file under an id of your own is kept unless `--with-own` deletes it too, as the
window's Delete does (ADR 0344):

```sh
encrust profiles reset-unreadable
encrust profiles reset-unreadable --with-own
```

Without `--kind`, `FILE` is a sliced file and its resin is taken in: whatever the file states
of exposure, transition, waits, lifts and light power, our defaults for the rest, set up on
the machine the file was cut for. That machine is found by the panel and the name the file
gives; where several share the panel and the file names none, `--printer ID` says which
(ADR 0293):

```sh
encrust profiles import vendor.goo
```

A `.cfgx` or `.ini` another program exported its resin as is read the same way, with its
colour, price and compensation besides, and set up on the machine it names; an `.ini` saved
without one needs `--printer ID`. Where each value comes from is in
`docs/formats/resin-profiles.md` (ADR 0295):

```sh
encrust profiles import "My resin.cfgx"
```

`--config FILE` holds flags written down: a key is a flag's long name, `true` the bare flag,
`false` nothing, an array the flag once per element. Keys at the top are the global flags,
a table names a subcommand, and a nested one a subcommand under it. A key fills in only a
flag that was not typed and has no variable set, so the command line always wins; written
flags behave as typed ones, so they win over a plate file or a project too, and a written
flag that conflicts with a typed one is an argument error. A key the command does not take
is an error, exit code 2.

```toml
no-progress = true

[slice]
printer = "elegoo-mars-4-ultra"
resin = "my-grey"
center = true

[printer.send]
start = true
```

`completions <shell>` prints the script for bash, zsh, fish, elvish or PowerShell:
`encrust completions zsh > ~/.zfunc/_encrust`. `cargo xtask man` writes a man page per
subcommand into `target/man`.

## Printers on the network

`printer discover` broadcasts for Elegoo boards, both SDCP generations, and asks each
`--address` directly for a network the broadcast does not cross. `printer status` and
`printer send` take a board's IP address, which is asked who it is before anything else;
`--wait` is how long it is given, in seconds. `send --start` starts the file once the
board has checked it. On a terminal a board's transfer draws a bar over its bytes; Ctrl-C
stops it between packets and exits 130.

```sh
encrust printer discover --address 10.0.4.17
encrust printer status 192.168.1.42 --json
encrust printer send model.goo 192.168.1.42 --start
```

A Prusa machine answers no broadcast and takes only `.sl1` and `.sl1s`. It is named by host
with `--prusalink`, and lets in a `--key`, or a `--password` under `--user` (`maker` unless set). Pass
them as `ENCRUST_PRUSALINK_KEY` or `ENCRUST_PRUSALINK_PASSWORD` rather than as flags, so they
stay out of the shell's history. Its upload is one request, so it has no bar and Ctrl-C lands
only before it starts (ADR 0152).

```sh
ENCRUST_PRUSALINK_PASSWORD=… encrust printer send model.sl1 sl1.local --prusalink --start
```

`pause`, `resume` and `stop` act on the print under way, as the printer card's buttons do;
a stopped print cannot be resumed, and this asks nothing first. `history` lists the last
prints an Elegoo board remembers (`--count`, 10 by default); a Prusa machine keeps none.
`camera on` prints the RTSP address an Elegoo board streams to, for a video player; `camera
off` turns it off. `picture -o FILE` saves the picture of the plate the printer keeps for the
print under way.

```sh
encrust printer pause 192.168.1.42
encrust printer history 192.168.1.42 --count 5
encrust printer camera 192.168.1.42 on
```

Under `--json`, `status` prints `printer`, `protocol` (`sdcp-3`, `sdcp-1` or `prusalink`),
`state`, `file`, `progress` from 0 to 1, `remaining_s` and `error`; `send` prints
`printer`, `protocol`, `file` as it landed and `started`; `discover` prints `printers`;
`pause`, `resume` and `stop` print `command`; `history` prints `prints`, each with `file`,
`ending`, `begin_unix_s`, `end_unix_s`, `layers_printed` and `layer_count`; `camera` prints
`on` and `url`; `picture` prints `output`.

## Batch

A folder of parts: every flag of `slice` applies to each model, one file and one
`<model>.json` each, plus `batch.json` over the lot. `--jobs` cuts several at once. A model
that fails does not stop the others; it is exit code 4 at the end:

```sh
encrust batch models/ \
  --printer elegoo-mars-4-ultra --center --orient --supports light -o out/
```

## The window

The window, optionally opening a model on startup:

```sh
cargo run -p encrust-app --bin encrust-gui -- model.stl
```

## `hollow-lab`

What each hollowing stage costs in time and in live bytes, over a matrix of settings, or
for one case named outright:

```sh
cargo run --release -p encrust-cli --bin hollow-lab -- model.stl [case index]
cargo run --release -p encrust-cli --bin hollow-lab -- model.stl --wall 0.3 --precision 0.5
```

## `orient-lab`

Each orientation goal's choice, four poses by hand — as the file stands on supports and
set on the plate, on its largest face, tilted 35° onto a corner — and, with
`--oracle N`, N directions spread over the sphere, all judged on what the pose actually
prints: the supports standing under it, then the stack cut and cured, read for contacts,
contacts left hanging, contacts on faces the file has looking up, layers, the hardest
pull on the film, islands and suction cups, and whether it fits the build volume. A goal
is chosen the way the window chooses it, by printing the search's finalists
(`docs/design/orientation.md`).

Every goal is held to the same rules against the best pose measured for the model,
whoever found it: no more cups and tearing pulls than the soundest pose; islands and
hanging contacts within a quarter and two of the fewest; nothing hanging under a pose
set on the plate; inside the build volume when anything is; its own reading within a
tenth of the best sound pose; not turned when standing as drawn was good enough; and
within three seconds a million faces. Each broken rule is named under the goal, and under
a goal that fails, its trace: the seconds its search and its proof took, and every
finalist it printed, on the coarse stack the proof ranks on, with what decided its place.
The run ends with the seconds all goals spent on each stage.

With no model it runs eight built-in bodies. `--corpus FILE` reads a TOML list of
models, each with what a person expects of it — `max-tilt-deg`, `on-plate`, for every
goal or one — which is held as one more rule. `--json FILE` keeps the run; `--baseline
FILE` sets it beside one kept before, measures again the poses that run's goals chose so
the best known pose is never forgotten, and exits non-zero when a goal that passed there
fails here. A goal no measured pose could pass is `unreachable` and left out of the
count; a corpus may `isolate` a model or `isolate-goals` one goal, which is measured and
shown but neither counted nor a regression. `--rejudge FILE` judges a kept run again by
the rules as they are now, measuring nothing.

```sh
cargo run --release -p encrust-cli --bin orient-lab
cargo run --release -p encrust-cli --bin orient-lab -- a.stl b.obj --printer elegoo-saturn-4-ultra --support medium
cargo run --release -p encrust-cli --bin orient-lab -- --corpus corpus.toml --json now.json --baseline before.json
```

## `support-lab`

Each model supported the way Generate supports it, cut and cured as it prints, and read
along three lines: what falls — contacts left with no support, islands, pieces standing on
nothing, tips thinner than hold at all; what marks the part — the scar the tips leave,
measured where the surface cuts each tip, tips on faces looking up in the pose the model
was drawn in, tips on material thinner than a millimetre, feet standing on the model; and
what is more or less than needed — how much of what it holds each tip carries, flat
underside and leaning underside each against its own limit, the rim of flat underside left
far from every tip, underside no tip reaches and no layer below carries, crowded tips and
tall trunks with nothing to brace them to. A model holds when nothing falls, and has
enough when it holds and no tip carries more than it holds.

`--profile` names the support profiles to bench, repeated for several, Light when none is
given. `--auto` names automatic profiles to bench the same way, each support sized by what
it holds as Generate does with Automatic on, and each tip read against the shape it was
built to; its row is named `auto:<id>`. `--hollow-wall-mm` hollows each model to a wall that
thick and stands its supports against the shell from outside. `--sweep` also supports each
model at shares of the profile's density and reports the least that is enough. `--flat-ratio`, `--slope-ratio`, `--min-tip-mm` and `--rim-gap-mm` set what a tip
holds, as prints of plates on tips measured it; `--coupon FILE --coupon-spec SPEC` writes
such a print instead of benching, laid out by a TOML file of tip shapes and the blocks
printed in them — balls, flat plates and leant plates on so many tips — with what the bench
expects of each specimen. `--corpus FILE` reads a
TOML list of models with how each stands — the axis that points up, the height it prints
at, its lean — and what a person expects of it. `--export DIR` writes each supported model
and markers at every place the run names; `--json FILE` keeps the run, and `--baseline
FILE` exits non-zero when anything got worse: a model that held fails or loses enough,
contacts or resin grow by more than 15 %, or more tips land on faces looking up or on thin
material.

```sh
cargo run --release -p encrust-cli --bin support-lab -- a.stl b.obj --profile light --profile medium
cargo run --release -p encrust-cli --bin support-lab -- a.stl --auto default --hollow-wall-mm 2
cargo run --release -p encrust-cli --bin support-lab -- --corpus corpus.toml --sweep --export out --json now.json --baseline before.json
cargo run --release -p encrust-cli --bin support-lab -- --coupon coupon.stl --coupon-spec coupon.toml
```
