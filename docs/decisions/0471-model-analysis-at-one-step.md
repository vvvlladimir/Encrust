# 0471. Model analysis runs at one step, and an edit knows no printer or resin

- **Status:** Accepted
- **Date:** 2026-11-19

## Context

ADR 0466 makes every change to a plate an `Edit` carried out by one `apply`. Three numbers
still differed by front end. Supports and the drain search slice the model: the edit path at
the slicer's default of 0.05 mm, the window and the command line at the machine's layer
height. The window's Arrange packs into the plate minus its inset, where the edit packed the
whole plate. The window's plug gap is the one the tool adds to the hole, where an edit's was
read as a bare size. One model then gave different supports, drains and layouts by how it was
asked for. The window is the reference.

The shipped resins are all measured at 0.05 mm, and supports read layers onto 0.1 mm cells
(`docs/design/supports.md`), so a finer step buys nothing there. Bench `analysis` in
`crates/core-engine/benches/plate.rs`, a 20 mm cube hollowed to a 2 mm wall, one run each
(median of ten samples, slicing included for the needs):

| Step, mm | `needs_on` | `suggest_drains_heard` |
|---|---|---|
| 0.02 | 72.0 ms | 254.8 ms |
| 0.05 | 37.5 ms | 102.8 ms |
| 0.10 | 26.3 ms | 52.2 ms |

## Decision

- **The document and the edits know no printer or resin.** Whatever depends on one comes into an
  edit as a number, worked out by the front end that knows the machine: the window, the CLI or
  MCP.
- **Model analysis runs at one step**, `core_engine::ANALYSIS_LAYER_MM` = 0.05 mm: supports,
  the drain search and the trapped-resin scan read the model there, whatever the machine's
  layer height. Before, a run took the machine's step; after, the constant. Slicing the print,
  Check's measure and the orient trial keep the machine's height, since they judge the print.
- **A plate area passed into an edit is the usable area**, the plate minus its inset, final
  as the window packs into it.
- **A plug gap in an edit is the final gap** between plug and hole, not a size the edit adds
  to.
- **Orient proves its poses on a printer on every path**, named by catalogue id in the edit
  (`printer`, `resin`), else the standard `elegoo-saturn-4-ultra` with the stock resin numbers
  (no resin ships), defined once in `printer-profiles`, read from the shipped catalogue alone. The window without a
  printer chosen, its panel and the edit rank alike; the command line does the same, on the stock
  support profile where it has no supports. An id the catalogue lacks is refused. The document stores no printer.

## Consequences

One model gives the same supports and drains from the window, the CLI and a link. A machine at
0.02 mm analyses 1.9 to 2.5 times faster than it did, at 0.1 mm 1.4 to 2 times slower; the
step is a constant a bench can move. A machine whose layer height differs from 0.05 mm no
longer sees its own layers in the analysis, only in the print. Reopen if a resin measured
above 0.1 mm ships, or if supports start to read finer than 0.1 mm cells.

## Alternatives considered

### Carry the machine's layer height in the document

Rejected: it brings the printer into every edit and makes one plate give different supports
by machine, which is what this removes.

### A finer step, 0.02 mm

Rejected: 1.9 to 2.5 times the cost on the bench for supports that read 0.1 mm cells.

### The step chosen, and what it costs

A fixed 0.05 mm is coarser than a 0.02 mm machine prints, so a feature a single layer thick
there can be missed by the analysis; Check's measure of the print still sees it.
