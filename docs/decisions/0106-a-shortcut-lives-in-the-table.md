# 0106. Keep every key in one table

- **Status:** Accepted
- **Date:** 2026-09-27

## Context

The window answered six keys, spelled out as constants beside the handler that read them.
Nothing listed them, so the only way to learn one was to read the source. Step 15e adds
some thirty keys — a digit per rail tool, the transport of the preview, the file commands
— and three places have to agree on each: the handler, the tooltip or menu row that names
it, and a sheet that lists them all. Three copies of the same fact drift, and the copy
that drifts silently is the one nobody reads.

## Decision

`encrust-app::shortcuts` owns a `const BINDINGS` of `(Group, Action, &[KeyboardShortcut])`
— one row per action, every chord that fires it — and everything else reads it: `pressed`
consumes the chords, `chord_text` writes them as keycaps, and `panels::shortcuts_sheet`
draws its two columns out of the same slice. `Action` is an enum answered by one exhaustive
`match` in `shortcuts::act`, which takes the borrowed `Window` every other cross-panel
action already takes. A pointer gesture that has no chord is a `Gesture` row: listed, read
by nothing.

The keycaps are ours rather than `Context::format_shortcut`, which spells its key names
out — "Questionmark", "Backslash", "Shift+Cmd+Z". macOS gets `⇧⌘Z`, `⌫`, `⏎`; the other
platforms get `Ctrl+Shift+Z` and words.

Two guards keep the keys honest. The handler stands down for `text_edit_focused` rather
than `egui_wants_keyboard_input`, which is true of any widget merely holding focus — a
button clicked a minute ago, after which every key died. And while the sheet is up it
takes both the keyboard and the pointer: `act` answers only Escape and `?`, and the
viewport reads an idle pointer, because it takes the raw input that no modal layer covers.

Keys cannot be rebound yet. The sheet is a modal, reached by `?`, F1, the keyboard glyph
in the title strip, the View menu and a row in the Settings list.

## Consequences

A key cannot exist unlisted: the table is the only way to bind one, and the sheet draws
whatever is in it. A binding cannot exist unanswered either, because `act` matches
`Action` exhaustively and the crate does not compile otherwise. Tests hold the rest: no
two rows share a chord, every tool has a key, the digits follow the rail's order.

`pressed` walks the table by modifier count, most first, because egui ignores an extra
Shift or Alt when matching a chord. The cost is one pass per count per frame, over a table
of tens of rows; a table of hundreds would want an index by key.

Rebinding is the signal to reopen this: user keys mean the chord stops being `const`, and
the sheet becomes an editor with conflict detection over it.

## Alternatives considered

### A page in Settings instead of a modal sheet

Where a rebinding UI will eventually live, and it costs nothing extra to draw the list
there. Rejected for now because the Settings screen replaces the plate: a user reaching
for a key while placing a model would lose the view of what they were doing, which is the
one thing a cheat sheet must not do. The Settings list still opens the sheet.

### Keys spelled where they are handled, and the sheet written by hand

No new module, and each panel keeps its own keys. Rejected: it is what the window did, and
it is why nothing listed them. A hand-written sheet is a second source of truth that no
test can hold to the first.

### The option that won, and what it costs

One table means one module that knows about tools, plates, projects, the preview transport
and slicing at once — `shortcuts::act` reaches into most of the window, and a reader
looking for what a key does no longer finds it beside the code it drives. It also makes
the table a merge point: two steps adding a key both edit the same lines.
