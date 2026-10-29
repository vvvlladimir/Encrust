# 0363. A peel has two stages, and the bottom layers a light-off delay of their own

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

A resin held one lift, one retract and one light-off delay. Resin presets for these machines
hold two stages of each peel — `*liftheight2`, `*liftspeed2`, `*dropheight2`, `*dropspeed2`,
bottom and normal — and a bottom light-off apart from the normal one, on 122 to 137 of its
194 machines. Its files carry them: `.ctb` v5 in the slicer info and the per-layer block,
`.goo` in the header and every layer, Anycubic's `EXTRA` and PWSZ `multi_state_paras`,
Creality's per-layer motion. Files we wrote for the same preset differed in every one of
those fields, and our print time ran short by the whole second stage.

Printed files fix the meaning. For a preset of bottom 5 + 3 mm and normal 2 + 4 mm
up, 3 and 4 mm slow on the way down, `.goo` states lift 5, lift 2 at the first speeds, then
3 and 4 at the second; retract 5 and 2 at the first speeds, then 3 and 4 at the second.
`.ctb` states 8 and 6 as the whole lift. The plate comes down as far as it went up, the last
stretch at the second speed.

## Decision

- `MaterialProfile::second_stage`, a `SecondStage` of eight fields: lift and retract distance
  and speed, bottom and normal. A lift's second stage climbs on past the first; a retract's
  is the slow last stretch of its whole travel, which is `retract_distance_mm` plus it.
  Zero distances leave one stage, the default, so every resin kept before reads as it printed.
- A bottom retract's first stage is derived: both bottom lifts less the second retract
  stage (`bottom_retract_distance_mm()`); the resin has no field for it, as no preset has one.
- `MaterialProfile::bottom_light_off_delay_s`, `None` for the normal delay.
- Both are keys of a printer's tuning table and of `[locked]`; a `.cfgx` import reads them.
  The print time counts the second stages and the bottom delay.
- A writer states them where its format has a place; a format with one
  stage gets the first. A reader of a Chitu container (`.ctb`, its encrypted form, `.fdg`,
  `.phz`) splits the second stage back off the summed climbs (`core_format::ChituPeel`).

## Consequences

Files carry the peel the resin asks for, and a two-speed peel shortens a print by what the
machines' presets save. The resin form grows a second-stage block; a machine that hides the
second stage locks it, like the tilting vats at zero. Every writer has to choose where the
second stage goes, and a format that has no place for it loses it silently.

## Alternatives considered

### Keep one stage and write the two stages' sums

The `.ctb` header holds sums; a `.goo` layer does not, and a sum cannot be split back into
two speeds.

### A retract's second stage added to the first

It reads like the lift, but printed files bring the plate back exactly as far as it
went: the second stage is inside that travel.

### The option that won, and what it costs

Nine fields more on every resin and in every table, eight of which mean nothing on a machine
with one stage, and a derived bottom retract a reader has to know about.
