# 0431. Turn a head or step a contact along its face before a support is left out, and say why

- **Status:** Accepted
- **Date:** 2026-11-11

## Context

The panel said only "N supports have no room to stand", and the command line counted them, so
neither a user nor a bench could tell a support whose head runs into the part from one with
nothing under it. Six models (bench figures, a mask, a duck, a bust), each with Medium and Auto,
left 30 supports out. All 30 were tips with no room for a straight head. With small pillars on,
such a tip stood headless, and `grow` offered it only a small pillar onto the part or a strut
straight down onto it. Over open air, or 0.3 mm over a steep face, neither stood, so the tip was
dropped. ADR 0339 had refused to turn a head for room because the cone, leant up to 60 degrees,
lay on the face. A contact stepped at most 0.3 mm (ADR 0329) to find room.

## Decision

- **Every left-out point carries why.** `core_supports::lost` names each point no tree
  carries: `HeadBlocked`, where no head fits, turned or not, and a headless tip found nothing;
  `FacesUp`, on a face looking up; `NoGround`, a head that fits with nothing within reach under
  it. `ModelSupports::lost` keeps them with the trees, the engine's `Stood::unheld` carries
  them, the panel prints one warning line per reason, and the command line prints and
  `--json`s them as `no_room_why`.
- **A refused head is turned, then cut.** `refitted_necks` tries the head 15 and 30 degrees off
  the way its face sends it, every 45 degrees round, then at 3/4 and 1/2 of `[top]`. No head
  turns past `MAX_HEAD_LEAN_DEG` = 33 degrees off its face's normal, the lean a steep face's
  head already has behind its pin. A head turns no further from plumb than the larger of its
  own and 45 degrees. A head leaving a face that looks up is never turned.
- **Then a contact steps along its face.** `moved` takes the nearest spots past 0.3 mm and
  within `MOVE_FAR_MM` = 1 mm, at the contact's height within 0.05 mm, on a face looking down.
  Each spot needs room for a straight head and must be reached under the face with no wall
  between. At most `MOVED_TRIES` = 24 are tried.
- **Where they run.** Without small pillars, `column` turns the head and then steps the
  contact. With them, a headless tip keeps its small pillar first. Once a family has grown,
  `grow` ends with `rescue`, which grows each column no tree carries again on its own: from a
  turned head, then from a stepped contact. It keeps the first that stands sound.

This continues ADR 0339's head and its 0.3 mm step, which still come first.

## Consequences

On the six models the left-out supports fall from 30 to 11, and two models lose none. All 11
that are left are `HeadBlocked`: faces looking down 0.3 to 0.5 mm over a 78-degree face of the
same model, where no head fits within a millimetre. Run times stay within noise, 1.3 to 5.5 s a
model, because `rescue` runs only for what is lost. Each lost tip costs at most 48 head sweeps
and 24 spots, each grown and checked once.

A turned head marks its face as a steep face's head already does, up to 9 per cent wider on one
side. A stepped contact holds a point up to 1 mm from where it was put, more than an island's
0.3 mm. Reopen if Check finds islands next to stepped contacts, or if prints show marks from
turned heads.

## Alternatives considered

### Lean a head past 33 degrees, as ADR 0339 rejected

This lays the cone on the face, which is the scar ADR 0339 removed. Within 33 degrees the pin
keeps the mark to the contact's circle.

### Turn heads in `neck_of` before small pillars

Tried first. Of 30 supports left out, 13 still were. A mask went from 6 to 19, because a turned
head took the place of small pillars that stood on the part, and its trunk then found no ground.

### Step contacts wherever there is room, through walls

This was what the first try did. A contact in a slot jumped its wall and held a different
pocket of the part. Only the path under the face keeps it holding what it was put to hold.

### The option that won, and what it costs

There are three more constants that no profile tunes. A hand-placed or automatic contact can
now stand up to 1 mm from where it was put, with no sign of it in the window. A pass after
growth gives a rescued support no chance to share a trunk, so it stands alone.
