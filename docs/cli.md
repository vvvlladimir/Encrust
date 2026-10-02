# The `slice` command line

Every flag the CLI answers, with the shape of an invocation that uses it. `--help` is
authoritative; this file is the worked examples. The window reaches the same code through
`encrust-app/src/job/`.

`--profile` and `--material` take a TOML path and win over the catalogue ids.
`--slice-window` trades memory for time on a dense stack (ADR 0066). The dev profile is
optimised (ADR 0037); every quoted timing is a release timing.

```sh
cargo run -p encrust-cli --bin slice -- --list-profiles
cargo run -p encrust-cli --bin slice -- --read plate.ctb           # open a sliced file
cargo run -p encrust-cli --bin slice -- model.stl -o out            # PNG stack

cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-4-ultra --resin standard-grey --center -o model.goo

cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-3-pro --center --ctb-version 5 -o model.ctb

cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-3-pro --center -o model.cbddlp     # or .photon
```

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

`--read` opens any of those containers, whoever wrote it, prints what the file states, and
then decodes every layer to check that it does: a header can be read from a file no machine
would print. A file whose name says nothing is recognised by its own first bytes (ADR 0149).

Hollowing, with an infill lattice in the cavity:

```sh
cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-4-ultra --hollow 2 --hollow-mode bottom-through \
  --infill hive --infill-size 5 --infill-density 0.15 --precision 0.5 -o model.goo
```

A hole into the surface nearest a point, cut with or without a cavity behind it, and the
check that says whether any resin is still stuck. `--check-drainage` asks for it alone:

```sh
cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-4-ultra --hollow 2 --drain 4 --drain-at 30,30,60 -o model.goo
```

The model's own texture pressed in as relief, as deep as the plate millimetres given.
Negative sinks it in. Needs a model carrying UVs and an image beside it:

```sh
cargo run --release -p encrust-cli --bin slice -- textured.obj \
  --printer elegoo-mars-4-ultra --relief 0.4 --precision 0.6 -o model.goo
```

Exposure of its own over a band of print height, repeatable. The bottom block keeps the
resin's ramp whatever a band says:

```sh
cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-4-ultra --exposure-at 4:6:9.5 -o model.goo
```

Thick layers on a vertical wall, thin on a shallow slope, against a cusp target. Needs a
printer profile whose `[firmware]` claims `variable_layer_height`:

```sh
cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-4-ultra --adaptive --cusp 0.03 --min-layer-height 0.02 -o model.goo
```

Every island taken out of the file, and whatever stood only on one:

```sh
cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-4-ultra --remove-islands -o model.goo
```

Turn the model the way it prints best before anything else is done to it, and build
supports to a shipped preset or to your own profile:

```sh
cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-4-ultra --orient --center -o model.goo

cargo run --release -p encrust-cli --bin slice -- model.stl \
  --printer elegoo-mars-4-ultra --supports medium --center -o model.goo
```

A folder of parts: every flag above applies to each model, one file and one
`<model>.json` each, plus `batch.json` over the lot. `--jobs` cuts several at once:

```sh
cargo run --release -p encrust-cli --bin slice -- models/ \
  --printer elegoo-mars-4-ultra --center --orient --supports light -o out/
```

The window, optionally opening a model on startup:

```sh
cargo run -p encrust-app --bin encrust -- model.stl
```

## `hollow-lab`

What each hollowing stage costs in time and in live bytes, over a matrix of settings, or
for one case named outright:

```sh
cargo run --release -p encrust-cli --bin hollow-lab -- model.stl [case index]
cargo run --release -p encrust-cli --bin hollow-lab -- model.stl --wall 0.3 --precision 0.5
```
