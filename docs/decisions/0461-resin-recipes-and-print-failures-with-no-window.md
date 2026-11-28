# 0461. A chat client sets up a resin, keeps a recipe and reads a failed print with no window

- **Status:** Accepted
- **Date:** 2026-11-18

## Context

A chat client beats a recipe only by what it knows and the engine does not: what the print is
for, which resin the user has, what came out. The window picks a recipe (ADR 0403), makes a
resin from stock numbers held to the printer's limits (ADR 0368) and writes a test print that
reads an exposure (ADR 0232); the command line could do none of it, so the client sent the user
to the window. The catalogue ships no resin, since an exposure is measured (ADR 0196). A
language model asked why a print cracked answers from memory, blind to the check's findings.

## Decision

- **Recipes:** `list_recipes` and `use_recipe` pick one by purpose. `estimate --recipe-out`
  writes the recipe a run took, each flag folded into its steps; `encrust recipes save` runs
  `core_recipe::validate` and keeps it under a fresh id, never overwriting; the server's
  `save_recipe` saves a job's steps or a TOML the client wrote.
- **Resin:** `import_resin` takes a file that printed well or another program's export.
  `encrust profiles new-resin` makes one from the maker's numbers, refusing a number past the
  printer's limits and naming one past its advice; the server's `create_resin`. The test prints
  move into **`core-samples`**, on `core-geometry` alone; `encrust test-print NAME` writes one.
- **Failures:** a client names symptoms from a fixed list; `failures.toml` in `encrust-cli` maps
  each to causes, a step, a change and a sign in the report; `encrust diagnose` answers those
  the report shows first. The server's `report_print` runs it and appends to `prints.jsonl`.

## Consequences

A user with a new bottle prints with no window, and a way of preparing found in a chat runs
again from `--recipe` or the window with no model. The engine, not the client, decides whether a
recipe runs or a number is taken. Advice after a print starts from the file's findings, and the
map is changed and tested like code. Reopen if the catalogue ships resins again, or photos prove
readable by a test.

## Alternatives considered

### The server writes `recipes/` and resin TOML itself

No new commands — but it could check neither the recipe nor the printer's limits.

### The client reads the photo and decides alone

Nothing to build, and nothing to test.

### The option that won, and what it costs

Three subcommands whose flags mirror the recipe, the resin and the symptom list, kept in step
with them by hand.
