# The `encrust` command line

Every subcommand, with the shape of an invocation that uses it. `encrust <command> --help` is
authoritative; this file is the worked examples and the contract a script can rely on. The
window reaches the same code through `core-engine`. In a checkout, `encrust` below is
`cargo run --release -p encrust-cli --bin encrust --`.

`--profile` and `--material` take a TOML path and win over the catalogue ids.
`--slice-window` trades memory for time on a dense stack (ADR 0066). The dev profile is
optimised (ADR 0037); every quoted timing is a release timing.

```sh
encrust profiles list
encrust profiles show elegoo-mars-4-ultra -o my-printer.toml   # a profile to start from
encrust info plate.ctb                       # open a sliced file
encrust inspect model.stl --printer elegoo-mars-4-ultra
encrust slice model.stl -o out               # PNG stack

encrust slice model.stl \
  --printer elegoo-mars-4-ultra --resin standard-grey --center -o model.goo

encrust slice model.stl \
  --printer elegoo-mars-3-pro --center --ctb-version 5 -o model.ctb

encrust slice model.stl \
  --printer elegoo-mars-3-pro --center -o model.cbddlp     # or .photon

encrust estimate model.stl --printer elegoo-mars-4-ultra   # time, resin, price; writes nothing
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

| Code | Meaning |
|---|---|
| 0 | Done |
| 1 | The run failed |
| 2 | The arguments were wrong |
| 3 | `--strict`, and a model has defects or does not fit |
| 4 | A batch in which at least one model failed |
| 130 | Stopped by Ctrl-C |

On a terminal `slice` draws a bar over the layers on stderr, and `batch` one over the
models; `--no-progress`, `--quiet` and `--json` turn it off. The first Ctrl-C stops the run
between layers and removes the file it was writing; a second one exits at once.

The output extension picks the format (ADR 0047): `.goo`, `.ctb`, `.cbddlp`, `.photon`,
`.sl1`, `.sl1s`, `.zip`, `.cxdlp`, `.svgx`, `.cws`, one of the seven Anycubic extensions in
`docs/formats/anycubic.md`, or a name with no extension for a directory of PNGs.
`--ctb-version` is read only by a `.ctb` name. A `.cxdlp` name is written at version 3 unless
the printer profile names version 4, as the Halot Mage line does (ADR 0168).

Each container pays for grey differently. A `.cbddlp` carries it in eight one-bit passes, so
it is eight times the layer data of the others (ADR 0146). An Anycubic file carries four bits
of it in one pass, which costs nothing but loses an edge dimmer than 16 (ADR 0147). An `.sl1`
carries all eight bits and loses nothing, because its layers are PNGs, and so do a `.zip` and
a `.cws`. A `.cxdlp` carries eight bits at version 3 and seven at version 4. An `.svgx` carries
none at all: its layers are polygons, and a pixel is lit or dark at half coverage (ADR 0169).

An `.sl1` states one exposure and one layer height for the whole stack, so `--exposure-at`
and `--adaptive` are refused rather than flattened under an `.sl1` name (ADR 0148). A `.zip`
is the other archive and carries both, because every number it states is per layer in its
`run.gcode` (ADR 0167).

`info` opens any of those containers, whoever wrote it, prints what the file states, and
then decodes every layer to check that it does: a header can be read from a file no machine
would print. A file whose name says nothing is recognised by its own first bytes (ADR 0149).

Hollowing, with an infill lattice in the cavity:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --hollow 2 --hollow-mode bottom-through \
  --infill hive --infill-size 5 --infill-density 0.15 --precision 0.5 -o model.goo
```

A hole into the surface nearest a point, cut with or without a cavity behind it, and the
check that says whether any resin is still stuck. `--check-drainage` asks for it alone:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --hollow 2 --drain 4 --drain-at 30,30,60 -o model.goo
```

The model's own texture pressed in as relief, as deep as the plate millimetres given.
Negative sinks it in. Needs a model carrying UVs and an image beside it:

```sh
encrust slice textured.obj \
  --printer elegoo-mars-4-ultra --relief 0.4 --precision 0.6 -o model.goo
```

Exposure of its own over a band of print height, repeatable. The bottom block keeps the
resin's ramp whatever a band says:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --exposure-at 4:6:9.5 -o model.goo
```

Thick layers on a vertical wall, thin on a shallow slope, against a cusp target. Needs a
printer profile whose `[firmware]` claims `variable_layer_height`:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --adaptive --cusp 0.03 --min-layer-height 0.02 -o model.goo
```

Every island taken out of the file, and whatever stood only on one:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --remove-islands -o model.goo
```

Turn the model the way it prints best before anything else is done to it, and build
supports to a shipped preset or to your own profile:

```sh
encrust slice model.stl \
  --printer elegoo-mars-4-ultra --orient --center -o model.goo

encrust slice model.stl \
  --printer elegoo-mars-4-ultra --supports medium --center -o model.goo
```

## Plates: several models, a plate file, a project

Several models go on one plate. `--arrange` spreads them, biggest first; without it each
stands where its file put it. `--center` and `--drain-at` point at one model, so they are
refused for several:

```sh
encrust slice a.stl b.stl c.stl --printer elegoo-mars-4-ultra --arrange --supports light -o plate.goo
```

A plate file says the same per model. Paths are relative to it, every key is optional but
`path`, and a key it does not know is an error:

```toml
printer = "elegoo-mars-4-ultra"
resin = "standard-grey"
layer_height_mm = 0.05
arrange = false                 # true spreads every model; it cannot be mixed with position

[[model]]
path = "a.stl"
rotate = [0, 0, 45]             # degrees around X, Y, Z
scale = 1.2                     # or [x, y, z]
position = [60, 40]             # where the middle of the footprint goes, plate mm
supports = "medium"             # light, medium or heavy

[[model]]
path = "b.stl"
hollow = { wall_mm = 2.0, mode = "bottom-through" }
```

A project saved by the window slices as it was saved, its cavities and supports built again
(ADR 0178). It carries its own models, so a flag that shapes one — `--hollow`, `--supports`,
`--rotate` and the rest — is refused; `--printer`, `--resin` and `--layer-height` replace the
project's own:

```sh
encrust slice plate.toml -o plate.goo
encrust slice plate.encrust --resin standard-grey -o plate.goo
```

A flag wins over the plate file or project, which wins over the profiles' own numbers.

## Estimate, info and convert

`estimate` takes what `slice` takes and writes nothing. It prints what the stack is and,
with a printer, what the print takes: time, resin by volume and weight, price where the
resin has one, and the layers likely to fail. `--strict` fails it on the same defects.

`info --layer N --png out.png` takes one layer out of a sliced file, counted from one as a
printer's screen counts.

`convert` writes a sliced file again in the container its output names, for the printer
given. The masks are copied, never resampled, so the printer's panel must be the size the
file was drawn for, in pixels and, where the file records it, in millimetres; the layer heights and every exposure are the file's own, and the resin
gives the lifts, waits and price. A stack of varying layer heights is refused for now, and
the new file's previews are blank (ADR 0180).

## Profiles, a config file, completions

`profiles show <id>` prints a printer, resin or support profile as the TOML it is kept in, or
writes it with `-o`; edited, it is what `--profile` and `--material` take. `--kind` picks
one when two kinds share an id.

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
resin = "standard-grey"
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
`--wait` is how long it is given, in seconds. `send --start` starts the file once it has
landed. On a terminal a board's transfer draws a bar over its bytes; Ctrl-C stops it between
packets and exits 130.

```sh
encrust printer discover --address 10.0.4.17
encrust printer status 192.168.1.42 --json
encrust printer send model.goo 192.168.1.42 --start
```

A Prusa machine answers no broadcast and takes only `.sl1` and `.sl1s`. It is named by host
with `--prusalink`, and lets in a key, or a password under `--user` (`maker` unless set). Pass
them as `ENCRUST_PRUSALINK_KEY` or `ENCRUST_PRUSALINK_PASSWORD` rather than as flags, so they
stay out of the shell's history. Its upload is one request, so it has no bar and Ctrl-C lands
only before it starts (ADR 0152).

```sh
ENCRUST_PRUSALINK_PASSWORD=… encrust printer send model.sl1 sl1.local --prusalink --start
```

Under `--json`, `status` prints `printer`, `protocol` (`sdcp-3`, `sdcp-1` or `prusalink`),
`state`, `file`, `progress` from 0 to 1, `remaining_s` and `error`; `send` prints
`printer`, `protocol`, `file` as it landed and `started`; `discover` prints `printers`.

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
