# 0338. Make the supports in one card, its click modes in its heading

- **Status:** Accepted
- **Date:** 2026-10-24

## Context

Supports were made in three tools of the rail (ADR 0218): Supports grew and picked them, Paint
placed, painted and held the groups, Shape held the profile's parts. Each repeated the
automatic-or-by-hand switch and the profile with its two save buttons (ADR 0325), and none
worked without another: moving picked supports to a group meant choosing the group in Paint
and pressing the button in Supports, and tuning a tip and growing again meant Shape, then
Supports, then Shape. On a short screen every switch also meant scrolling past the repeated
header. A group was a second list beside the profile list, though every group is built to one
profile and named for it.

## Decision

**One Supports card, with four click modes as icons in its heading**: Pick, Place, Area (Paint
or Block, chosen inside it; alt erases whichever is in hand, so there is no eraser) and Shape.
`Tool::Paint` and `Tool::Shape` stay as the tools a mode sets, since the viewport's click
follows the tool, but are no buttons of the rail; `Tool::card` maps them to Supports. The
digits follow the shorter rail.

**One picker under the heading is both the preset and the groups.** Closed, it names the
preset in hand, or the one the group in hand is built to, marked edited once it differs from
what is saved, with the group's colour and count. Open, it lists Auto with the shapes it built
under it, then the presets by hand; a line that has supports standing is that group, with its
eye and bin. A group is named for its preset, never for anything else. Beside it Save and a
bin for the group in hand. The card starts on the automatic preset.

**A bin asks twice**: the first click turns it red, a second click on it removes, and moving
off it disarms it. No dialog asks before a group goes. Grow is pinned at the foot with the bin
for every support beside it, Fill painted areas over it while there is paint or the brush is
in hand, and the cavity's button above that.

## Consequences

Picking, placing, painting, shaping and growing need no trip to another tool, and the header
costs one line. Group creation by hand and renaming are gone: a group appears when a preset
first holds supports. A group made under another name by an older project shows its preset's
name. Erasing paint and blocks in one stroke now takes two, one per layer.

## Alternatives considered

### Keep three tools, sharing a header and a foot

Rejected: placing and picking would still sit in different tools, and the profile and groups
would still be two lists.

### The click modes on a toolbar over the viewport

Rejected: it covers the model, and the modes belong to the card whose blocks they change.

### One card, and what it costs

A mode now changes the tool underneath, so a shortcut to Paint or Shape no longer exists; the
modes are a click in the heading away. The card's blocks change with the mode, which a user
of the old Paint tool must learn once.
