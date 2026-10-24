# 0254. Ship presets tuned on prints, and let Heavy stand fewer, thicker supports

- **Status:** Superseded by 0323
- **Date:** 2026-10-13

## Context

The shipped Light, Medium and Heavy took the field's measurements (ADR 0131), with our own
density and overhang angle laid over them so that a heavier preset also placed more
supports: 0.6, 1.0 and 2.5. Printing with them gave a new set of numbers for all three:
thinner tips and pillars, narrower pads, bracing on in every preset, and a Heavy that stands
on the plate alone, does not branch, and places fewer supports than Light rather than more.

Two of the new numbers break rules the code holds the presets to. Light's 2 mm lift and
Heavy's 5 mm are short of the pad, flare, top segment and shortest pillar under the lowest
contact (ADR 0237): 3.575 mm and 5.5 mm. And Heavy's 0.5 density is under Light's 0.6.

## Decision

**The three presets take the printed numbers**, in the assets and in the constructors
alike (ADR 0132).

**The lift rule stands, and the lift follows it**: Light lifts a part 3.6 mm and Heavy
5.5 mm. Even with its top segment cut short, Light's lowest contact would have no room
for the shortest pillar at 2 mm.

**A heavier preset is thicker, not denser.** Heavy keeps 0.5. What still grows with the
preset is the overhang angle it holds to — 55, 45, 35 — and the width of what it builds.
The order of the densities in ADR 0131 is withdrawn.

## Consequences

- Heavy puts fewer supports under a part than Light, each wider and standing on the plate:
  with `land_on_model` off, a contact over the part leans for the plate or is not built.
- A part printed with Light stands 1.6 mm higher than before, about 30 layers at 0.05 mm.
- Medium, which a new object starts with, braces its trunks and has a 7 mm pad.
- The density test now varies density on one profile instead of comparing presets.

Reopen if a Heavy run leaves an overhang under-supported that Light held: the answer then
is a denser Heavy, not a thicker one.

## Alternatives considered

### Take the numbers verbatim, lifts included

Exactly what was printed. Rejected because Light's lowest contact would not stand at all,
and a preset that cannot support the bottom of the part it lifted is a broken preset.

### Keep Heavy the densest

Preserves ADR 0131's ordering. Rejected because the prints asked for fewer, thicker
supports under heavy parts, and three presets that differ only in thickness were the
outcome 0131 wanted to avoid — the overhang angle still keeps them apart.

### The option that won, and what it costs

Two lifts differ from the printed profiles, so a user comparing their file with ours finds
3.6 and 5.5 where they wrote 2 and 5.
