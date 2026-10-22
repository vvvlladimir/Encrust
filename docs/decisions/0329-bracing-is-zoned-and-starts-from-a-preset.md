# 0329. Zone the bracing up a pair, and start it from a preset

- **Status:** Accepted
- **Date:** 2026-10-22

## Context

ADR 0043 ties a pair of trunks with one cross every `rise_mm` from `start_height_mm` up, as
high as both stand. A cross holds both ways, but not every level needs that. Near the plate
a trunk is short and stiff, and what sways is the top, just under the joint where its
branches leave; a uniform cross spends resin all the way up to hold those two places. The
four numbers also leave a user to find, by trial, a combination that suits the part, and
the window shows only the numbers.

## Decision

**Bracing is zoned.** `[bracing]` gains `bottom` and `top`, each a `pattern` (`zigzag` or
`cross`) and a number of `levels`, and `middle`, a pattern. Up a pair, `bottom` takes its
levels up from the first brace and `top` takes its levels down from the ceiling — the lower
of the two root nodes, so the top level ends where the trunk splits. `middle` fills what is
left between them with whole rises. Where there is no room for every zone, the bottom fills
first, then the top. A zigzag is one brace a level, leaning the other way on the next and
carrying on from one zone into the next; a cross is two. `core_supports::brace_levels`
places the levels, and the generator and the window's drawing both use it.

A profile written before the zones loads with both zones empty and a crossed middle, which
is ADR 0043's bracing exactly.

**Presets are numbers.** `BracingPreset` names four sets — Standard, Strong, Tall and Small —
of diameter, span, start, rise and zones. Picking one writes its numbers into the profile;
the profile never stores which one was picked. The window shows the presets while the
profile's numbers are one of them, and the numbers otherwise; a button switches between the
two, which is the window's state and not the profile's.

| | Diameter | Start | Bottom | Middle | Top |
|---|---|---|---|---|---|
| Standard | 0.9 | 1 | zigzag ×1 | zigzag | cross ×1 |
| Strong | 1.0 | 0 | cross ×1 | cross | cross ×1 |
| Tall | 2.0 | 1 | cross ×1 | zigzag | cross ×2 |
| Small | 0.8 | 1 | cross ×1 | cross | cross ×2 |

All four span 10 mm and rise 5 mm. A brace is still never wider than either trunk where it
meets it (ADR 0242), so Tall's 2 mm is a ceiling the trunk usually sets lower.

## Consequences

A zigzag middle halves the braces between the two ends that need them. The top level now
meets the trunk at its joint, where before it stopped up to one rise short. The presets
brace more densely than the shipped profiles' own numbers, which are left as they were and
show as numbers. Five more fields in the profile file, all optional.

Reopen if a zigzag pair is seen to twist on the peel where a crossed one did not.

## Alternatives considered

### Store the preset's name in the profile

It would let the window show the preset after an edit, but then the name and the numbers
can disagree, and the profile is the numbers. Recognising a preset by its numbers costs
nothing and cannot drift.

### A separate catalogue of bracing presets

Users could save their own, but the profile is already that: a saved profile carries its
bracing. A second catalogue is a second place to keep the same numbers.

### The option that won, and what it costs

A preset is four fixed sets in code, so changing one changes which profiles show as it, and
a profile a user tuned one tenth off a preset shows as numbers.
