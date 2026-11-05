# Recipes

A recipe is the order, the conditions and the settings Prepare for print and `--prepare`
run a model through, written down as a TOML file (ADR 0403). `core-recipe` reads it, checks
it and answers, step by step, whether to run that step and with what; each front end runs
the step with its own machinery: the window by the tools' own jobs, the command line by
the flags it resolves (ADR 0402). The autopilot is the built-in recipe `prepare`.

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
| `drain` | `diameter_mm`, `vent`, `plugs` | Drain's Auto, `--drain-at auto` |
| `supports` | `profile` (catalogue id, `auto` the default), `cavity` | Grow, `--supports` |
| `check` | `retry = { while, max }` | Check, the report |
| `arrange` | `spacing_mm` | Arrange plate, `--arrange` |
| `save` | `folder`, `name` (ADR 0387's tags), `send` | Slice to, `-o`, `encrust printer send` |
| `gate` | `stop_if`, `message` | ends the run, the plate put back |
| `pause` | `message` | the window waits for Continue; the command line says it and goes on |

`keep_solid_under_mm` keeps the wall solid wherever the model is thinner than that
(`core_volume`'s Keep solid, fitted). `check`'s retry supports every island again while
`while` holds, at most `max` times, `max` no more than 5.

## Order

A step may only follow what it works on. Each work step appears once; `gate`, `pause` and
`check` as often as wanted. Checked when the recipe is read:

| Step | Comes after, where present |
|---|---|
| `hollow` | `orient`, `repair` |
| `drain` | `hollow` |
| `supports` | `orient`, `hollow`, `drain` |
| `check` | `supports` |
| `arrange` | `supports` |
| `save` | every work step |

`repair` and `orient` take either order; nothing else is free.

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
| `trapped` | pockets of resin with no way out | `drain` or `check` |
| `islands`, `peel_n`, `support_ml` | the cut stack | `check` |

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

A step that fails ends the run by `on_fail`: `stop` puts the plate back as the run found it
(the window's one undo entry, ADR 0402), `skip` goes on to the next step. Several models run
each step together; a gate that stops on one stops the run.

## Where recipes live

The built-in ones in `assets/recipes/`, compiled into `core-recipe`; the user's beside
their profiles in `recipes/`, a file each, its stem the id. `core-recipe` reads no files:
the front ends hand it the text.
