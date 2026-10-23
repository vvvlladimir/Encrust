# 0334. Stand supports against what prints, and hold a cavity up by a run of its own

- **Status:** Accepted
- **Date:** 2026-10-23

## Context

Every support question — where a run places, where a column lands, what a beam runs into —
was asked of the model as it was imported, solid. A hollowed model prints as its shell, the
cavity appended wound inward (ADR 0059), and a drilled one as that shell with its holes
taken out exactly (ADR 0322), so a run never saw a cavity's ceiling or the crown of a hole,
and a support under either would have landed in material that is not there. The painted and
blocked patches are sets of the model's faces (ADR 0092, 0093), and the drilled mesh numbers
its faces afresh. A cavity often wants holding up apart from the outside, after it.

## Decision

- **The printed surface.** Supports stand against the model drilled where its holes have
  been taken out, else its shell once hollowed, else the model. The window keeps that
  surface's hierarchy beside the model, built once per surface, and rebuilds the supports
  when it changes; a run cuts and reads it, columns land and beams are swept against it,
  and opening a project drills the shell it holds the same way and meshes against that.
- **Patches stay on the model.** A fill reads the model's faces, and a blocked patch is
  asked by point — `Placed::blocks_at`, within 0.01 mm of the patch — rather than by face,
  so it holds on any surface however it is numbered.
- **Inside is a run of its own.** A contact is inside when the surface nearest it bounds the
  cavity, or the lattice standing in it. **Grow the supports** keeps only the outside;
  **Support the cavity**, shown while a model aimed at has a cavity, keeps only the inside,
  sized by the profile in hand like any run. Once anything stands inside, the button is
  **Delete the cavity supports** and takes those away, frozen trees whose every tip is
  inside included.
- **A run keeps to its side while it places**, as a blocked patch keeps it off: a contact
  on the far side is never taken, so it holds up nothing near it, and a support already
  standing there is no seed.
- **A line keeps to its side too.** A line drawn with shift (ADR 0332) takes each point to
  the nearest face on the side of the wall its first support stands on, so a chord that
  dips into a thin wall is not taken through it onto the cavity.
- **Seen through the section.** A part of a support is taken by a click between where the
  section lets the ray in and the first printed surface it meets, or the cap where the cut
  stands in material; so a support inside a cavity the section opens can be picked.

## Consequences

A cavity, a drain hole and any bore are held up like every other underside, by the profile
chosen above, and a support under the outside bites only as deep as the real wall. One more
hierarchy per hollowed or drilled model is held in memory and built on the window's thread
when the surface changes. Until a hole has been taken out exactly, or where the engine
refused it, the hole is not seen.

## Alternatives considered

### Keep supports on a shell that begins with the model's faces

Kept painted faces numbered alike, but a drilled model, a mould and an open floor would get
no supports in what they cut.

### Switches for each side and a group for the inside

A settings block for what one button does: run the inside after the outside, by the profile
in hand, and take it away again.

### The option that won, and what it costs

A face within a hundredth of a millimetre of a blocked patch counts as blocked, a frozen
tree with tips on both sides of the wall belongs to neither button, and a run on a hollow
model asks one nearest-face query of every contact it considers. The button counts the
supports inside again only when a support or the surface changed, one query each.
