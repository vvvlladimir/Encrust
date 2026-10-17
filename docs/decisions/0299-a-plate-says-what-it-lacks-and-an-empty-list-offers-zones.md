# 0299. Say at the stage's foot what a plate lacks, and offer the first machine and resin as zones

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

A first run has no machine and no resin (ADR 0158, 0196), and the window refuses to slice
without both. Nothing said so until **Slice** was pressed. The way to both is the Machine
and resin window (ADR 0220), opened from the top bar's chip, which a newcomer has no reason
to look at. Inside it, an empty machine list was one grey line, a machine with no resin an
empty table under two small buttons (ADR 0197), and a machine taken out of the library was
installed but not printed on until **Use this machine** was found as well.

## Decision

**The window opens only when asked for.** A plate with no machine, no resin or neither
says which in a notice at the foot of the stage (ADR 0217), with **Set up** opening Machine
and resin. The notice stays for as long as something is missing, and the start page shows
none.

**An empty list is two large zones filling its room:**

- With no machine of the user's own, the machine list below its search line is **Add a
  machine** over **Import a machine**, each half its height, in place of the rows and the
  buttons under them.
- With no resin on the machine, its Resins tab is **Add a resin** beside **Import a resin**,
  each half its width, in place of the filters, the table and the action bar. **Add a
  resin** asks between a new one and the pool only when the pool has something to offer.

**A file dropped on the open window is a profile, never a model.** A sliced file or another
program's resin profile can only be a resin; any other file is a resin while a machine's
Resins tab is in view and a machine otherwise. The zones say so.

**A plate with no machine takes the one picked, installed or imported in the window**, and
a plate with no resin takes the first its machine gains (`adopt_default_resin`), so neither
needs a second click.

## Consequences

- A newcomer sees what is missing wherever the plate is, and is one press from a screen
  where the next step is the largest thing on it.
- *Add a resin* with an empty pool puts a resin at the stock numbers under the plate the
  moment it is made, as *Use this resin* always could. The form is open for the numbers,
  but nothing stops a slice before they are typed. Reopen if first prints fail on
  untouched resins.
- The panel pictures are taken on a plate with a resin, so the notice shows only in the
  picture of the notices.
- Exposure calibration, a print of a test and an answer that corrects the exposure, is not
  part of this and is a decision of its own.

## Alternatives considered

### Open the window by itself when the start page is left

Built first, with two steps ticked off at the top of the machine list. It lost on use: a
window that opens unasked interrupts the user who only wanted to look at a model, and its
steps repeated, smaller, what the list and the tabs beside them already showed.

### A checklist on the start page

Machine, resin and a test print in place of the start page until done. It lost because the
start page is for opening work, and a user who opens a model straight away skips past it.

### The option that won, and what it costs

A notice holds part of the stage's foot until the plate has both, and nothing walks a
newcomer from the machine to the resin: after the machine they must notice the Resins tab's
zones themselves, which the tab being open on them makes likely but not certain.
