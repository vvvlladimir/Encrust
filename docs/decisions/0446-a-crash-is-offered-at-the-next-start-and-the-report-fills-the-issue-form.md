# 0446. Offer the panic a run closed on at the next start, and carry the report into the issue form

- **Status:** Accepted
- **Date:** 2026-11-14

## Context

The report sheet (ADR 0212) carries what the user types and what the window knows while it
is open. A panic that closes the window loses its message and backtrace: the process is gone
before anyone can open the sheet, and the desktop builds have no console to read it from.
With the window in beta, a crash is the bug most worth hearing about and the one a user is
least able to describe. Panics also happen that are not crashes: `core-csg` catches the
boolean engine's own and carries on.

The issue form is prefilled through its URL, which GitHub refuses past about eight kilobytes,
so ADR 0212 put the report on the clipboard and only the short fields in the URL. Users did
not paste it: the line telling them so stood in Encrust's window, not in the form they were
looking at.

## Decision

`encrust_app::report::crash` installs a panic hook before the window opens. Each panic writes
`last-crash.txt` beside the user's profile folder: the version, the thread, the location, the
message scrubbed of paths and addresses as the report scrubs a failure, and a forced
backtrace. The last panic of a run overwrites any before it. A clean exit deletes the file,
so a panic caught and lived through leaves nothing. At the next start the window reads and
deletes the file and opens the report sheet with it; the report carries it whole under its
own heading, its backtrace cut to Encrust's own frames, and the issue form's log field takes
it without the backtrace. The browser build keeps no file and records nothing.

`bug.yml` has a *Report from Encrust* field. The URL fills it with what the window adds —
everything but the two typed answers, which have fields of their own — when that fits under
6 000 characters, and otherwise with a line asking for the clipboard to be pasted there. To
fit more often the report names a shipped, unchanged printer by its id instead of its TOML,
and carries only the tool values that differ from a fresh window's. Nothing is sent, as ADR
0212 holds.

## Consequences

A crash reaches the user as a report already written, once. Release builds carry no line
tables, so the backtrace names functions, not lines. A run killed from outside after a caught
panic, or one that ends without a panic (a GPU driver abort, a signal), offers a stale panic
or none. A crash in the browser is still lost; a reload is where the user would see a sheet,
and the tab keeps no file.

A reader of a report sees only the settings that were changed and looks the shipped printer
up by id; a profile edited after the build shipped is still carried whole.

Reopen if crashes without a panic turn out to be the common case, or a second window per
process makes "a clean exit" ambiguous.

## Alternatives considered

### Keep the first panic of a run

The first panic is usually the cause, but a caught panic from the boolean engine would then
mask the one that closed the window.

### Send the crash somewhere

A crash service would hear about every crash, not only the ones a user hands over; ADR 0212
rejects sending anything for reasons that apply here unchanged.

### Shorten the URL with a paste service or a gist

Either holds a report of any length behind a short link, but it is a send: the report would
leave the machine before the user read it, which ADR 0212 rules out.

### The option that won, and what it costs

A file written from a panic hook and a delete in `on_exit`. What it misses: crashes with no
panic, and the browser. A stale file is possible when the process is killed after a caught
panic, which shows the user a panic that did not close the window. A report too long for the
URL still depends on the user pasting it, now asked for in the form itself.
