# 0334. Leave every panel as it was between runs, and keep automatic sizing with the tools

- **Status:** Accepted; keeping a widget's choice until the file is deleted is superseded by 0345
- **Date:** 2026-10-23

## Context

ADR 0192 made `ToolSettings` what the preferences file, a project and the undo stack all
carry, but automatic sizing (ADR 0325) was left out of it: the switch between **Automatic**
and **By hand**, the automatic profile chosen and its edited numbers lived on the tool
alone. A restart opened **By hand** on the shipped numbers, and Undo stepped over them.

The rest of what a panel shows was not remembered either. A section folded, the Shape
tool's part tab, the Check filter, a scale lock, and what a click does in Paint, Drain and
Measure were kept in egui's memory or on the tool for the run only.

## Decision

- **Automatic sizing is a tool value.** `SupportState` carries `sizing: Option<AutoSizing>`,
  the automatic profile with its numbers as edited and whether a run uses it, so a project
  writes it, the preferences remember it and a switch or an edit is one undo entry. A file
  without it reads as none chosen.
- **A panel's own choices are remembered, not undone.** Like the Relief mode (ADR 0192),
  what a click places in Paint and Drain, what Measure catches and draws, and which axes
  the handles follow go into the preferences as `Modes`, beside the tool values.
- **A widget's own choices go there too, by its id.** A section's fold, a tab of a panel
  and a lock beside a field read and write `ui::kept`, a map from egui id to a JSON value
  that the preferences file carries as `kept`.

## Consequences

Closing the window and opening it again leaves every panel as it was, and a project opened
elsewhere sizes its supports as it was saved. Undo now takes back the sizing switch.

An egui id is a hash of where the widget stands. A widget that moves, a title that changes
with the language, or a new egui that hashes otherwise finds nothing kept and opens on its
default; the map keeps the old entry until the file is deleted. Reopen this if the map
grows past a few hundred entries.

## Alternatives considered

### egui's own persistence

eframe saves egui's memory when its `persistence` feature is on. Rejected because it pulls
in a second store and its format beside `preferences.json`, and the browser build keeps
the preferences in the page's storage already.

### A typed field per choice

Clear to read, but every fold and tab would need a name, a field and its plumbing. Kept
for the modes a tool holds, which are few and read by more than the panel.

### The option that won, and what it costs

The kept map is untyped: a value that no longer reads as what the widget asks for is
taken as never chosen, and nothing says so.
