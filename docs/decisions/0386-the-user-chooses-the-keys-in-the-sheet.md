# 0386. Let the user choose the keys, in the sheet that lists them

- **Status:** Accepted
- **Date:** 2026-10-31

## Context

ADR 0106 put every key in a `const BINDINGS` table and named rebinding as the signal to
reopen it: user keys mean a chord stops being `const`, and the sheet becomes an editor
with conflict detection. Users now ask to set their own keys and have every place that
shows or answers a key follow them: the handler, tooltips, menus, Find and the sheet.

## Decision

`BINDINGS` stays the table of defaults. A `Keymap` in `shortcuts::keymap` holds only the
actions the user changed, by a stable `Action::id`, and `shortcuts::chords` answers the
user's keys where there are any, the table's otherwise. Every reader goes through
`chords`, so nothing else changed.

The keymap lives in a thread-local beside the palette's, read with no `Window` in hand —
tooltips are drawn from many panels that have none. `Preferences` keeps it as
`{"undo": ["Cmd+U"]}`, `Cmd` meaning Ctrl off the Mac; an unknown action or chord is
dropped on reading, and an action set back to its defaults is not written.

The sheet becomes a window in the Settings frame: groups down the left, the actions on the
right, each key a button that records a chord — modifiers and a key. `shortcuts::take`
hands every key to the recording before anything else reads it, the desktop's copy, cut
and paste as their chords; each chord pressed replaces the last, and Done keeps it. Esc
stops and keeps nothing; Tab is not recorded. In a browser every chord pressed while
recording is kept from the page, so Ctrl D records rather than bookmarks. An action holds
at most two keys. A chord another action answers is asked about under the row:
Replace takes it from that action, Cancel drops it. Pointer gestures are listed, never
rebound.

## Consequences

A key the user set is in force everywhere the moment it is set, and is remembered with
the other preferences, in a file or the page's storage alike. Esc and Tab cannot be
recorded, nor a chord the browser keeps for itself (Ctrl W, Ctrl T); an action that loses its key to
another can be given it back by its reset arrow, and every key by Reset every key. The
command line has no keys, so there is nothing for it to follow.

## Alternatives considered

### A keymap carried in `Window` and passed to every tooltip

No global state. Rejected: some forty calls in a dozen panels would take a new argument
for a value that changes a few times a run, and the key hook runs before any `Window` is
borrowed.

### Refusing a chord already taken

Simpler, with no question to ask. Rejected: moving a key from one action to another
would take two trips through the sheet.

### The option that won, and what it costs

A thread-local is state a test has to put back, and a reader of `chords` cannot see from
its signature that the answer depends on it. `holder` and `assign` walk the whole table
for each recorded chord; at tens of rows that is nothing, at hundreds it would want an
index by key.
