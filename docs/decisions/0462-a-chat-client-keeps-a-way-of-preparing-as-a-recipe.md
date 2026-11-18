# 0462. A chat client keeps a way of preparing as a recipe the engine checked

- **Status:** Proposed
- **Date:** 2026-11-18

## Context

A chat client prepares a job step by step, and each revision keeps the `core-recipe` its
check ran, every edit folded into its steps (ADR 0459). What it learns about a user's prints —
busts face up, no hollow under 30 mm — is lost when the chat ends, unless it lands where the
window and the command line look: the user's `recipes/` folder (ADR 0405). A user may also
describe a way of preparing in words, which only the client can turn into steps. A recipe
the client wrote may name a variable no model has or a step out of order; the window lists
such a file but cannot run it.

## Decision

- **`encrust recipes save PATH [--name NAME]`** reads a recipe file, renames it when asked,
  runs `core_recipe::validate` on it, and keeps it under a fresh id from its name, past the
  user's ids and the built-in ones, as the window's **New recipe** does. A recipe `validate`
  refuses is not kept, and the error says why.
- **`save_recipe`** of `encrust-mcp` takes a name and either a job (and revision) — whose
  kept steps are saved — or a TOML the client wrote in the layout `list_recipes` shows. It
  runs `recipes save` on a scratch copy; a refusal is a tool error, for the client to fix and
  save again.

## Consequences

A way of preparing found in a chat runs again in the window and from `--recipe` with no
language model. The engine, not the client, decides whether a recipe runs, so a recipe
from words is as safe as one built in **Settings → Recipes**. Saving never overwrites: a
second save is a second recipe, and the user deletes the extra in the window. Reopen if
clients should replace a recipe in place.

## Alternatives considered

### The server writes the file into `recipes/` itself

No new command — but `encrust-mcp` depends on no crate of the workspace (ADR 0455), so it
could neither check the recipe nor find the catalogue's folder as the window does.

### Keep a recipe that reads but does not validate, as the window does for a hand edit

The window keeps such a file so a user can fix it there; a client can fix it in the chat
before anything is written, and a listed recipe that cannot run is a failure the user would
meet later, without the chat.

### The option that won, and what it costs

One more subcommand, and a client that writes a recipe must learn its layout from the
examples `list_recipes` gives; a mistake costs a round trip.
