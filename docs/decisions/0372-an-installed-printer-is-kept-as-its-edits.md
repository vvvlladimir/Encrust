# 0372. An installed printer is kept as its edits over the shipped one

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

Adding a machine from the library writes a whole copy of the shipped profile into the user's
directory (ADR 0158), and that copy hides the shipped one by id. A later release's
correction never reaches it, edited or not: every LD-002H installed under v0.5.0 keeps the
`front_edge` ADR 0369 has since turned. ADR 0196 only names the difference and offers to
write the shipped profile back, which throws away what the user did change.

A whole copy cannot tell an edit from a value the release it came from shipped, and the
machines are few enough to ask their owners to make an edit again.

## Decision

The file of a printer whose id ships holds `edits_over_shipped = true` and only the fields
that differ from the shipped profile; `unset` lists the dotted keys the user left at their
default where the shipped one states a value. Reading it lays those over the printer this
build ships, field by field, tables merged and arrays replaced. `locked` is never an edit
(ADR 0301); the printers a machine sends to are always one, as nothing ships them.

A file of a shipped id without the marker is a whole copy an older release left: it is put
back to the shipped printer, rewritten as no edits, the old file kept beside it as
`<id>.toml.bak`. A file of an id this build does not ship is the user's own printer and is
read as it stands.

## Consequences

- An update reaches every field the user left alone, new fields included, with no step of
  theirs; an edit stays theirs whatever later releases ship under it.
- *Use the shipped profile* keeps working: it saves no edits.
- Edits over an id a later build stops shipping no longer read; the file is left out with
  `EditsOverNothing`, as a broken file is (ADR 0344).
- What an owner changed in v0.5.0 or before is undone once; the `.bak` holds it, and the
  release notes say so. A whole file written by hand over a shipped id is treated alike: to
  change a shipped printer by hand, write its edits under the marker.
- v0.5.0 and older cannot read the new file, so a downgrade loses the machine from the list;
  the `.bak` is the old file. `--profile` on such a file fails, as it is not a whole profile.

## Alternatives considered

### A whole copy with the fingerprint of the shipped profile it was taken from

Old releases read it, but an edited copy then takes no correction at all, and a release
that changes a profile has to merge and rewrite every copy.

### The printers v0.5.0 shipped as a baseline

What differs from them would be the owner's edits, kept through the move. It costs 102 more
profiles in the build, and a copy older than v0.5.0 would keep what v0.4.0 to v0.5.0 changed
as edits nobody made; few machines are installed to keep that for.

### The option that won, and what it costs

The user's file of a shipped printer is no longer a profile on its own, and the merge is
done on every read; a typo in an edit fails loudly only once laid over the shipped profile.
