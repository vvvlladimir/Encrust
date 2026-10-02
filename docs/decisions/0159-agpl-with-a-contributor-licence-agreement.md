# 0159. Publish under AGPL-3.0 with a contributor licence agreement

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

The repository is private and, until now, inconsistently licensed: `Cargo.toml` declared
`MPL-2.0` and no `LICENSE` file existed, so nothing was actually granted to anybody. Going
public forces the question, because the choice binds every later one.

Three facts shape it. Encrust competes with two subscription products whose paid tiers are
automation — supports, hollowing, risk detection — and with vendor slicers paid for by the
hardware they sell. It needs printer profiles that only owners of those printers can
produce, which is why it has to be open at all. And it must be able to carry its own cost
one day: the nearest comparable project is AGPL with no paid tier, has around 700 000
release downloads, and has not met a goal of 25 recurring sponsors.

MPL-2.0 is weak copyleft per file. A competitor or a printer vendor could take the cores,
build a closed product on them, and return only edits to our own files. That is the one
outcome that would make the work unpaid *and* unattributed.

## Decision

The public source is licensed `AGPL-3.0-only`, with the full text in `LICENSE` and the
same identifier in `[workspace.package]`.

Every contribution is covered by `.github/CLA.md`, enforced by the `cla` workflow: the
contributor keeps copyright and grants the maintainer a licence that is explicitly not
limited to the project's current terms. That is what allows a second, commercial licence
to be sold later without asking every contributor again.

Four things are free and open forever, and no later licence changes that: slicing,
rasterisation and writing a printer file; basic supports, hollowing, drains and island
detection; every printer and resin profile; sending a file to a printer. Anything paid can
only ever be something that is not in that list.

## Consequences

A fork, a vendor build and a hosted service all have to stay open. The profiles and fixes
the project depends on come back. The licence is the one this corner of 3D printing already
understands, and it was the basis on which the Software
Freedom Conservancy held a printer vendor to account in 2025 — a precedent worth standing behind.

The costs are real. AGPL is refused outright by some corporate legal departments, so a
printer vendor wanting Encrust inside a closed product must buy an exception rather than
just take it; the CLA is what makes selling that exception possible, and it is also
friction on a first-time contributor and a thing some people refuse on principle. A
contribution accepted without a signature would remove the ability to dual-licence, so the
workflow blocks the merge rather than trusting the review.

Reopen this if the AGPL is demonstrably costing more vendor conversations than exceptions
it sells, or if the project ever accepts an unsigned contribution it cannot remove.

## Alternatives considered

### Keep MPL-2.0

Friendliest to the vendors we most want to talk to, and already written in `Cargo.toml`.
Rejected because it permits exactly the failure mode that matters: a closed competitor
built on these cores, with nothing owed back.

### GPL-3.0

Closes the fork hole without frightening lawyers as much as the AGPL does. Rejected as
strictly weaker here for no gain — a slicer is heading towards hosted preparation, and
GPL-3.0 does not reach that.

### A business source licence that opens after a few years

Would protect against direct cloning now. Rejected because it is not open source, and the
contributors and profiles this project needs are exactly the people who would decline.

### AGPL-3.0-only plus a CLA, and what it costs

`-only` rather than `-or-later`: the terms cannot be changed from under the project by a
future licence revision, at the price of needing a relicensing decision to adopt one. The
honest cost of the whole option is that the CLA asks every contributor for something
before their first patch lands, in exchange for a revenue path that does not exist yet.
