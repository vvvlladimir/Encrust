# 0099. Unsaved work is a digest of the manifest, asked for only at the door

- **Status:** Accepted
- **Date:** 2026-09-26

## Context

Now that the plate goes into a file (ADR 0097), closing the window can throw away an
evening. The window has to know whether what is in front of the user matches what is on
disk.

The plate is edited from everywhere: twenty-odd panels, the gizmo, every tool, every
background job that lands. There is no single write path to hang a flag on. The one thing
that already summarises the whole plate is the project capture — and it clones every
painted patch, which is hundreds of kilobytes on a painted model. Running it per frame to
light an indicator would be the most expensive thing the window does.

## Decision

Dirtiness is the digest of the serialised manifest. It is recorded when a project is
written, read or emptied, and compared **only when the window is asked to close**.

A close request over a plate that does not match its digest is cancelled, and a modal
asks: save and close, close anyway, or keep working. Closing anyway sets a flag, so the
close that follows is not questioned again.

There is no live unsaved marker in the title strip.

## Consequences

Every edit is covered, including ones made by a tool written later, because nothing has
to remember to set a flag: the manifest is the definition of what a project holds, so
anything the file records is something the guard notices, and anything it does not record
is correctly ignored.

The user gets no warning that work is unsaved until they try to leave. On a crash or a
power cut there is nothing at all — this buys a prompt, not a recovery file.

Reopen this when autosave arrives: a periodic write needs the digest on a timer, and at
that point the cost of a capture has to be paid on a schedule anyway, which is also what
would make a live title marker free.

## Alternatives considered

### A dirty flag set by every mutation

Exact, and free to read per frame. It has to be set in every panel and every job, and the
one place it is forgotten is a silent data loss rather than a visible bug.

### Reuse the undo history's scene fingerprint

Already computed, already cheap, already per frame. It covers the scene and nothing else:
a changed layer height, a new exposure band or a retuned support group would all close
without a word. It also hashes mesh pointers and object ids, both of which change on load,
so a freshly opened project would read as dirty.

### The option that won, and what it costs

No indicator until the user reaches for the close button, which is the least helpful
moment to be told. It also means the answer is computed once, under a modal, so a plate
large enough for the capture to be slow shows that as a hitch exactly when the user is
trying to leave.
