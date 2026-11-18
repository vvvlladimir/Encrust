# 0464. A chat client makes a resin from its maker's numbers, held to the printer's limits

- **Status:** Proposed
- **Date:** 2026-11-18

## Context

A print needs the user's resin; the catalogue ships none, since an exposure is measured,
not transcribed (ADR 0196). A chat client takes one in from a file that printed well or
another slicer's export (ADR 0460), but a user with a new bottle has neither, only the
numbers on it or on its maker's page. The window's **New resin** makes one from the stock
numbers for the user to edit, and will not let a number past the printer's `[limits]` be
typed (ADR 0368). The command line had no way to make one, so the client sent the user to the
window.

## Decision

- **`encrust profiles new-resin`** takes the printer, a name, the exposure, the bottom
  exposure and the bottom layers, and where stated the layer height, light-off delay, lift
  distance and speed, retract speed, transition layers and density; the rest are the stock
  numbers **New resin** starts from. It is kept under a fresh id and set up on that printer
  alone, by the catalogue's `import_resin`, as an imported resin is.
- **Held to the printer's limits:** a number past what the printer takes is refused, naming
  each; one past what its maker advises is kept and named under `advised_against`.
- **`create_resin`** of `encrust-mcp` runs it on the numbers the user gave. The instructions
  tell the client to ask for them and never to supply one, and to offer the exposure check.

## Consequences

A user with a new bottle prints with no window: the client asks for its numbers, makes the
resin and writes the exposure check that tells whether they are right. A wrong number the
printer refuses never reaches a file. The client could still mistype one the printer takes;
the exposure check and `report_print` (ADR 0463) are what catch it. Reopen if the catalogue
ships resins again.

## Alternatives considered

### The client writes a resin TOML and imports it

No new command — but the client would have to learn the profile's layout, and an import
checks none of the printer's limits.

### Refuse what the printer advises against as well

Safer on paper — but makers state numbers outside a printer's advised span, and the window
keeps them with a warning too.

### The option that won, and what it costs

One more subcommand whose flags mirror the resin's main fields, to keep in step when a field
is added; the less common ones are set in the window.
