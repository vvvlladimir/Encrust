# Recipes

A recipe is the order, the conditions and the settings Prepare for print and `--prepare`
run a model through, written down as a TOML file (ADR 0403). `core-recipe` reads it, checks
it and answers, step by step, whether to run that step and with what; each front end runs
the step with its own machinery: the window by the tools' own jobs, the command line by
the call each flag makes (`--recipe`). The autopilot is the built-in recipe `prepare`.

## The file

```toml
name = "Busts"               # what the picker shows
format = 1                   # the version of this layout

[vars]                       # numbers the conditions and settings may name
big_ml = 30

[[step]]
do = "orient"
when = "!turned"             # optional; absent runs always
on_fail = "stop"             # or "skip"; default "stop"
set = { goal = "least-peel", max_tilt_deg = 30 }
choose = [                   # the first whose `when` holds is merged over `set`
  { when = "height_mm > 120", set = { goal = "shortest-print" } },
]

[[step]]
do = "hollow"
when = "sound && !hollow && saves_ml >= big_ml"
set = { wall_mm = "= clamp(height_mm / 50, 1.5, 3)" }

[[step]]
do = "gate"
stop_if = "trapped > 0"
message = "Resin stays trapped; drill it by hand"
```

A setting is a number, a boolean, a word, or an expression: a string starting with `=`.
A setting left out takes the engine's default, never what a panel holds, so a recipe
prepares the same in the window and on the command line. `enabled = false` keeps a step in
the file and out of the run.

## Steps

The window and the command line run every kind; what each kind sets:

| `do` | Settings | Runs as |
|---|---|---|
| `repair` | `method`: `repair`, `close-holes`, `wrap`, `deep` | Repair, `--repair` |
| `orient` | `goal`: `auto`, `fewest-supports`, `least-peel`, `shortest-print`; `max_tilt_deg` | Auto-orient, `--orient` |
| `hollow` | `wall_mm`, `mode`: `inside`, `outside`, `open-floor`; `infill`; `keep_solid_under_mm` | Hollow, `--hollow` |
| `drain` | `at`: `auto` or `points`; `points`; `diameter_mm`, `depth_mm`, `vent`, `plugs` | Drain's Auto or a click, `--drain-at` |
| `supports` | `profile` (catalogue id, `auto` the default), `cavity` | Grow, `--supports` |
| `check` | `retry = { while, max }` | Check, the report |
| `arrange` | `spacing_mm` | Arrange plate, `--arrange` |
| `save` | `folder`, `name` (ADR 0387's tags), `send` | Slice to, `-o`, `encrust printer send` |
| `gate` | `stop_if`, `message` | ends the run, the plate put back |
| `pause` | `message` | the window waits for Continue; the command line says it and goes on |

`check`, `arrange` and `save` work on the plate as a whole, once a step. `save` writes into
`folder`, `~` the home (`save_folder`), named by `name`; `send` fails the step with no printer
to send to. Left out, the window takes its preferences, where it saves sliced files (else the
home) and its file name template, which the command line cannot know: it takes the home and
`DEFAULT_TEMPLATE`. That is the one default the front ends hold apart.

A drain's `diameter_mm` and `depth_mm` left out are `core_volume`'s `HoleSize::default()`,
3 mm across and 3 mm deep, in both front ends; on the command line `--drain` and
`--drain-depth`, typed, stand in for them, their defaults that same hole.

`keep_solid_under_mm` keeps the wall solid wherever the model is thinner than that
(`core_volume`'s Keep solid, fitted). `check`'s retry supports every island again while
`while` holds, at most `max` times, `max` no more than 5: one support under each island the
stack shows (`core_engine::island_holds`), added to what the model stands on as Check's
Support every island adds it, nothing standing grown again.

`drain` with `at = "auto"` (the default) is the proven search of ADR 0401 and needs a
cavity; with `at = "points"` it drills a hole at each of `points`, `[x, y, z]` fractions of
the placed box (`[0.5, 0.5, 0]` the middle of the floor), moved to the nearest face as
`--drain-at` moves one. Drilled before `hollow` such a hole is a recess with the wall kept
around it (ADR 0346); after, it drains.

## Order

Any order, any step as often as wanted: each step works on the model as the steps before
it left it. A step with nothing to work on is skipped and says why, as `drain` with
`at = "auto"` on a model with no cavity. Where a later step undoes an earlier one, the
recipe is still read and the plan shows a note under the later step:

| Later step | Over an earlier | The note |
|---|---|---|
| `orient` | `supports` | the supports are taken off; a later `supports` grows them again |
| `orient` | `drain` with `auto` | the holes were chosen for the earlier pose |
| `hollow` | `supports` | the new cavity stands on nothing inside |
| `repair` | `hollow`, `drain` | a repair remeshes the shell and may close the holes |
| `supports` | `supports` | the later supports replace the earlier |
| `save` | — | none; a `save` before the end writes the plate as it is then |

## Variables

Measured on the model as it stands when the step is reached. A name measured only after a
step (the last column) is refused in a condition placed before that step.

| Name | Meaning | Known after |
|---|---|---|
| `height_mm`, `width_mm`, `depth_mm` | the placed box | — |
| `volume_ml`, `surface_cm2`, `faces` | the mesh | — |
| `sound`, `defects` | the import's diagnosis | — |
| `turned` | the user turned it (`--rotate`, a turn in the window) | — |
| `hollow` | it has a cavity | — |
| `saves_ml`, `saves_share` | ADR 0393's estimate at the `hollow` step's wall, 2 mm without one | — |
| `worth_hollowing` | the estimate past `MIN_SAVED_MM3` and `MIN_SAVED_SHARE` | — |
| `fits` | it fits the build volume with what stands under it | — |
| `trapped` | pockets of resin with no way out, by Drain's Check drainage; 0 with no cavity | `drain` or `check` |
| `islands` | the cut stack's islands the model hangs from, and those that hang from none | `check` |
| `peel_n`, `support_ml` | the stack's hardest pull on the film, N; the model's supports, ml | `check` |

A `[vars]` name may not shadow one of these.

## Conditions

```
expr  := or
or    := and ("||" and)*
and   := not ("&&" not)*
not   := "!" not | cmp
cmp   := sum (("<" | "<=" | ">" | ">=" | "==" | "!=") sum)?
sum   := prod (("+" | "-") prod)*
prod  := unary (("*" | "/") unary)*
unary := "-" unary | atom
atom  := number | "true" | "false" | name | name "(" expr ("," expr)* ")" | "(" expr ")"
```

Values are numbers (`Scalar`) and booleans, never mixed: `height_mm && sound` is refused when
read. Functions: `min`, `max`, `abs`, `clamp(x, lo, hi)`. Division by zero is an error at the
step, not infinity. No loops, no assignment, no strings: a condition always ends.

## Running

`core_recipe::decide(plan, index, metrics)` answers one step: `Run` with the settings
resolved, `Skip` with why, `Stop` with the gate's message, or `Pause`. Each carries what it
read, which the plan before a run and the report after it show:
`hollow — run: saves_ml 42 ≥ big_ml 30`. Before a run, a step whose condition reads
a name not yet measured is shown as *decided when reached*.

On the command line a flag typed for a model, or a plate file's key for it, wins over the
setting it shapes in every step of its kind: `--hollow 3` is every hollow step's `wall_mm`,
`--supports light` every supports step's `profile`, `--no-drain-vent` every drain's `vent`;
`--rotate` makes `turned` true. `--orient`, `--repair`, `--hollow` and `--supports` also run
every step of their kind whatever its `when` (`decide_forced`), at the step, not at import; a
recipe with no enabled step of that kind refuses the flag, naming it.

A step that fails ends the run by `on_fail`: `stop` puts the plate back as the run found it
(the window's one undo entry, ADR 0402), `skip` goes on to the next step. Several models run
each step together; a gate that stops on one stops the run.

## Where recipes live

The built-in ones in `assets/recipes/`, compiled into `core-recipe`; the user's beside
their profiles in `recipes/`, a file each, its stem the id. `core-recipe` reads no files:
the front ends hand it the text.
