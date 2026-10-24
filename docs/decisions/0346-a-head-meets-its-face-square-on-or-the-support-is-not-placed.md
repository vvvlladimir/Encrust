# 0346. Meet every face square on with the head, or leave the support out

- **Status:** Accepted; a contact with no room for a head is left to a small pillar by 0348, the head along the normal whatever way it looks is superseded by 0350
- **Date:** 2026-10-24

## Context

A head followed its face's normal only within 60 degrees of straight down (ADR 0344); off a
steeper face, a wall or a slope looking up it left at 60 degrees, a face looking straight
up sent it straight down through the part, and one that met the model turned up to 60
degrees off its direction or kept it (ADR 0328). Each of those meets the face slantwise:
the widening cone lies on the face past the tip, and the mark it cures grows from the
contact's width to several times it at 60 degrees off the normal. Users saw such supports
on gentle faces that could not be broken off cleanly, and asked that a support touch only
by its tip, its mark the contact's width, or not be placed.

## Decision

A head leaves its face along the face's normal, whatever way that looks. A support whose
head, swept from a hair off the face, meets the model, or rises so nearly straight up that
a trunk from its neck stands less than a pillar's and a contact's radius off the contact,
is not placed: `neck_of` answers `None` and `columns` leaves the point out, which the panel
counts as having no room to stand. Everything under the neck bends and leans by the rules
it had. The headless strut of ADR 0331 stays, only in a profile with small pillars, which
ship no wider than their contact.

This supersedes ADR 0344, the turning of a head and keeping a head that turns nowhere in
ADR 0328, and a head straight down off a flat top.

## Consequences

Every head's mark is the contact's width (ADR 0345). Contacts in a crevice, beside a lip
or on a face looking up that once stood on a slanted head now stand nowhere, and are
reported; a contact under a wall leaves level and is held up by what its neck bends into.
`neck_of` sweeps one head per contact instead of up to 33 turned ones. Reopen if automatic
runs leave faces that need holding unheld in numbers.

## Alternatives considered

### Let the head lean a few degrees off the normal

A cone from the tip's equator lies on the face at any lean; ten degrees widens the mark by
about 5 per cent. A limit would be a constant no profile could reach, for supports a user
asked not to have.

### Keep turning the head, and shrink the contact by the lean

Keeps the mark's width but not its shape, and a narrower tip holds less than its profile
says.

### The option that won, and what it costs

Faces that ADR 0328 kept held are now left unheld, which that ADR rejected; the user has
since said a support that cannot touch by its tip alone is worse than none.
