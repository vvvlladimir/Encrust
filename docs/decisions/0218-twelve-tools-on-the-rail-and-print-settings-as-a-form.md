# 0218. Twelve tools on the rail, and the print settings as a form

- **Status:** Accepted
- **Date:** 2026-10-10

## Context

After ADR 0217 the rail held eight tools, but several of them were two or three tools in
one. Select carried the gizmo, the transform fields, Orient to face and Auto-orient;
Supports carried the automatic run, the brush, the groups, the profile and forty fields
of support shape; the Preview column showed the issues, the peel, the opened file and the
print settings whatever tool was lit. The v2 design gives each of those its own entry and
shows the open tool in the inspector in every view.

## Decision

`workspace::Tool` is twelve rail tools in four groups, plus one form:

- **Place:** Select (what is picked, nothing to change), Position (move, rotate, scale,
  Place, Orient, Lay flat, Mirror, and the gizmo), Measure.
- **Modify:** Hollow, Drain, Cut, Relief.
- **Supports:** Supports (the automatic run, its overhang and spacing, and a click picks
  the parts of supports already standing), Paint (Place, Paint and Block, the fill and
  the groups), Shape (the profile, then Tip, Body, Base, Raft and Brace).
- **Finish:** Check (the verdict, the issues, the peel), Export (the file, the machines
  in reach, and Slice, Send or Start print).
- **Print settings** is a form, not on the rail: the top bar's chip opens it. It holds
  the layers, the exposure, the bands, the edge smoothing, the waits and the motion.

The inspector is a heading — glyph, name, help, the tool's one fact — the tool's sections,
and the tool's one action pinned to the foot where it has one. It shows the open tool in
the model view and in both layer views; picking a tool no longer switches the view, which
the layer strip alone does. Check offers a press that cuts the layers when there are none.

Only Drain, Check and Export carry a dot on the rail, and only while they need attention:
resin is trapped, the check found something, a sent file waits to be started.

Digits 1 to 9 and 0 pick the first ten tools in rail order; Check, Export and the print
settings have no key. Controls the design has and the window cannot do yet are drawn
greyed out with "Not available yet" on hover (`ui::later`), each with a
`TODO(step-8)` naming the work.

This supersedes ADR 0217's return to the model when a tool is pressed, ADR 0118's gizmo
on Select, and ADR 0105's Layers tool and its Mirror in the plate's row of actions.

## Consequences

- A tool panel answers one question; Supports no longer scrolls past forty fields to its
  button.
- Placing a support by hand moves from Supports to Paint, and picking a standing support's
  parts is what a click does in Supports rather than a fourth brush.
- Select no longer moves anything: the gizmo is Position's.
- The greyed controls show where the design is going and do nothing; the reopen signal is
  a user taking one for a broken button.
- Check, Export and the print settings are reached by the pointer alone. Reopen if users
  ask for keys there.

## Alternatives considered

### Letters for the tools, as the design has them

V, T, M, H, D, C, E, S, B, K and P read as the tool's name, and would key all twelve.
The digits were kept because they are what the window already answered, and a letter
fights the fields' typing once more keys land in the table.

### A tool that switches the view it needs

Check could open the layers on its own, and the earlier tools brought the model back.
Rejected: the view jumping on a tool press is the jump ADR 0217 set out to remove.

### The option that won, and what it costs

Twelve tools are more to learn than eight, and four of them used to be one panel each, so
a user who knew where a field was has to find it again. Two of the tools have no key, and
the greyed controls take room before they do anything.
