# 0192. A tool value is remembered between runs and taken back on its own entry

- **Status:** Accepted
- **Date:** 2026-10-06

## Context

`preferences.json` carried the machine, the resin and the output format (ADR 0105), and
nothing a tool is set to: a wall thickness typed into the Hollow panel was gone at the
next start, as was every brush radius, hole size, cut plane and array gap. A project
carries those values (ADR 0191), but a plate that was never saved opens on defaults.

`History` restores a `Scene` clone and finds an edit by hashing what the scene carries
(ADR 0086). A tool's values are not in the scene, so `Cmd+Z` stepped over a changed
setting to the model edit before it. ADR 0086 named this as the signal to reopen it: a
tool whose state genuinely does not belong to the scene.

The two have to agree on one set of values, or the file remembers one thing and the undo
stack another.

## Decision

`tool_settings::ToolSettings` is that set, and it is the only statement of it: the slicing
panel's values, the support groups and what paints them, the hollow, drain, cut and relief
numbers, and the array. It is built from the tools in hand and applied back to them, and
the project manifest's own state types are its fields, so a project, the preferences file
and the undo stack carry the same values the same way.

- The window writes the record to `preferences.json` whenever it changes on a settled
  frame — a drag over a slider is one write, not one per frame — and applies it on startup
  after the printer and the resin, so what the user last cut at stands over what the
  profile was measured at.
- `History` takes a second kind of entry. A plate entry restores a `Scene`, a tool entry
  restores a `ToolSettings`, and one change of either is one entry. A tool entry leaves
  the scene alone, so a slider moved while a run is landing a shell cannot put the plate
  back.
- Choosing a printer or resin is not an edit. `Slicing::chosen` counts how many have been
  taken into hand, and the history reads the values a new profile brought with it as its
  baseline instead of recording them: undoing a resin would otherwise leave the plate cut
  at a height that resin was never measured at.
- The first frame observed is the baseline, so opening a project, starting a new plate or
  reading the preferences file records nothing to undo.

The manifest has no entry for the Relief tool, and this does not add one: its values are
remembered between runs, and a project opens with whatever the window is set to.

## Consequences

Every value a panel holds survives a restart and answers `Cmd+Z` and `Cmd+Shift+Z`, and
there is one place to add the next tool's values to.

The record is built and compared on every settled frame, which clones the exposure bands
and the support groups — kilobytes beside a scene hash that already walks every support
point. A value left out of `ToolSettings` is silently neither remembered nor undoable,
which is the honesty `fingerprint` already asks for in ADR 0086 and now asks for twice.

Relief is the one asymmetry: a project opened on another machine presses with the local
window's amplitude. Reopen this when the format is next versioned, by giving the manifest
a relief entry and dropping the field from this record's own definition.

## Alternatives considered

### One snapshot of the scene and the settings together

Simpler: one entry, one kind, the existing hash extended. Rejected because the jobs land
meshes a frame or two after the edit that asked for them. Undoing a slider moved while a
shell is landing would put back a scene from before it and lose the shell, and a project's
own tool values would have to be undone against a scene they never belonged to.

### Leave settings off the undo stack

What a configuration screen usually does, and the panels would need no second entry kind.
Rejected because the tool panels are where most of the work happens: a wall thickness and
a cut height are edits of what will be printed, not preferences about the window.

### Remember the values in the project only

No preferences change at all, and a saved plate already opens as it was. Rejected because
a plate that was never saved — which is every first run — opens on defaults, which is the
report this came from.

### The decision above, and what it costs

Two kinds of entry on one stack, and a counter on `Slicing` that exists only so the
history can tell a profile's values from the user's. Both are state kept in step by
observation rather than by construction, and nothing fails loudly when a new field is left
out of either.
