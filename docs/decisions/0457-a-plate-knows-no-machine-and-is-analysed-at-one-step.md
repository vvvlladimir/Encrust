# 0457. A plate knows no machine, is analysed at one step, and is checked on the standard one

- **Status:** Accepted
- **Date:** 2026-11-19

## Context

With one `apply` (ADR 0456), three numbers still differed by front end: the edit path sliced
supports and drains at 0.05 mm, the window and the command line at the machine's layer height;
the window packed into the plate minus its inset, the edit into the whole plate; the window's
plug gap was the final gap, an edit's a size to add. A chat client's job also had to name a
printer before anything was done, and with none, `--center` centred the model on the origin,
so a plate point given before the user named a printer meant another place after.

The shipped resins are measured at 0.05 mm and supports read layers onto 0.1 mm cells. Bench
`analysis` in `crates/core-engine/benches/plate.rs`, a 20 mm cube hollowed to 2 mm:

| Step, mm | `needs_on` | `suggest_drains_heard` |
|---|---|---|
| 0.02 | 72.0 ms | 254.8 ms |
| 0.05 | 37.5 ms | 102.8 ms |
| 0.10 | 26.3 ms | 52.2 ms |

## Decision

- **The document and the edits know no machine.** What depends on one enters an edit as a
  final number from the front end: the usable plate area, the final plug gap.
- **Model analysis runs at `core_engine::ANALYSIS_LAYER_MM`, 0.05 mm**: supports, the drain
  search, the trapped-resin scan. Slicing, Check's measure and the orient trial keep the
  machine's height.
- **A pose is proved on a machine**: the printer and resin an edit names by catalogue id, else
  `STANDARD_PRINTER` (`elegoo-saturn-4-ultra`) with the stock resin numbers.
- **A job may start with no machine.** `start_job` takes both or neither; with neither, every
  check passes `--standard-machine`, which centres, arranges and checks on that printer and is
  refused by `slice`, `batch` and `convert`. `write_file` asks for the user's own.
- **Each check reports `models[].fit.bounds_mm`**, and a client takes plate points from it.

## Consequences

One model gives the same supports, drains and layout from every front end. Analysis is 1.9–2.5
times faster for a 0.02 mm machine, 1.4–2 times slower for 0.1 mm. An agent can open, cut and
prepare a model before the user knows the printer; a plate point keeps its place up to the file
while the user's plate is the standard size, and print figures until then are the stock
numbers'. Reopen if a resin above 0.1 mm ships, or supports read finer than 0.1 mm cells.

## Alternatives considered

### Carry the machine's layer height in the document

Brings the printer into every edit, and one plate would give different supports by machine.

### Translate plate points when the machine changes

Keeps them on a plate of another size, but not on the window's, which the server does not hold.

### The option that won, and what it costs

A feature one 0.02 mm layer thick can be missed by the analysis, and a client may read the
stock numbers' figures as the user's.
