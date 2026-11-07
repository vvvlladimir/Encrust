# 0405. Keep a user's recipe in the profile store, and look recipes up in core-recipe

- **Status:** Proposed
- **Date:** 2026-11-05

## Context

ADR 0403 puts a user's recipes beside their profiles in `recipes/`, a file a recipe, its
stem the id, and lets only the command line read them, straight from the folder. The window
now lists, runs, edits and saves them, and in a browser it has no folder: its profiles live
in the page's storage behind `ProfileStore` (ADR 0181). Both front ends must list the same
recipes in the same order and find an id the same way, a built-in id winning over a user's
stem. `core-recipe` reads no files and stands above `printer-profiles` (through
`core-engine`), so `printer-profiles` cannot read a recipe.

## Decision

`printer_profiles::Kind::Recipe` is a kind of the catalogue kept in `recipes/` through the
store, as the tools' presets are (ADR 0226): `Catalogue::recipes`, `save_recipe`,
`move_recipe`, `fresh_recipe_id`, export and `forget_user_copy` as for any kind. The
catalogue keeps a recipe as its text and never parses it. `core_recipe::library` lists the
built-in recipes then the user's `(id, text)` by id, each `Read` as valid, invalid with its
`RecipeError` and its steps, or unreadable; `core_recipe::find` resolves an id, built-in
first. The command line and the window hand it the catalogue's texts. A user's recipe is
saved, imported or renamed only when `validate` passes, under a stem no built-in holds.

## Consequences

The window's picker and editor work the same at the desk and in a browser, and a file the
command line reads is one the window wrote and the other way round. `encrust recipes list`
loads the profile catalogue to find the folder. A recipe that stops reading is listed with
its error, never one of the catalogue's unreadable files, so the question over those is not
asked for it. Reopen this if a recipe has to carry more than its text, such as a picture.

## Alternatives considered

### A recipe store of its own beside `ProfileStore`

A second trait and a second browser implementation for one more folder of TOML files,
which `ProfileStore` already keeps by kind.

### Parsing recipes in `printer-profiles`

It would need `core-recipe`, which stands above it: an edge the graph forbids.

### The catalogue keeping text, and what it costs

Each listing parses every user recipe again: a few small files a frame the window asks, so
the window keeps the listing until the catalogue changes. The catalogue cannot refuse a
recipe that does not read; the callers check before they save.
