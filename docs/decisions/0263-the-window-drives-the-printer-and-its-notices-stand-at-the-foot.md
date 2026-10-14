# 0263. Let the window drive a printer, and stack its notices at the foot of the stage

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

Since 0262 the window watches each printer it lists, but it can only start a file. A user
standing away from the machine wants to pause or stop a print, see what is on the plate,
look back at what printed, and know when the release film is due. A Saturn 4 Ultra serves
all of that: commands `129` to `131`, a history through `320` and `321` with a bitmap of
each plate, a camera through `386`, and `ReleaseFilm` against `ReleaseFilmMax`. A Prusa
machine serves the job's pause, resume, stop and thumbnail, and no history or camera.

The running jobs, a file waiting to be started and the last message were split between
the top and the foot of the stage (0217). Notices in two places read as two systems.

## Decision

- `net-sdcp`'s `Control` gains `pause`, `resume`, `stop`, `history`, `tasks` and `video`.
  A response's fields beside its `Ack` are kept for the command that waits on them.
  `picture` fetches a thumbnail, but only from a URL on the board's own address.
  `net-prusalink` gains `job`, `pause`, `resume`, `stop` and `picture`.
- `printer-link` gives the window one `control(wire, Command)`, plus `history`, `picture`
  and `camera`. A protocol without a history or a camera answers with nothing rather than
  an error. A Prusa machine printing nothing is `SendError::Idle`.
- `State` names the job under way, the film's count and rating, and whether there is a
  camera. The watch reads a board's attributes once per connection for the last two.
- Each request from the window runs on a thread of its own and opens its own connection,
  beside the watch's. A picture is decoded on that thread. The cards show the plate
  beside the progress, Pause or Resume, and Stop behind a second press. They also show the
  film's wear, the camera's stream address to copy, and the last five prints, read when
  their fold is first opened.
- The camera is RTSP carrying H.264. The window turns it on and shows the address, but
  does not decode the video.
- Every notice stands in one stack at the foot of the stage: running jobs, then a file
  waiting to be started, then the last message nearest the edge.

## Consequences

A user can watch and steer each print from the window, on any board that serves these
commands. Version 1 boards are sent the same command numbers, which the shared envelope
suggests but no recording confirms. A board that answers otherwise shows a refusal on its
card.

A command opens a connection beside the watch's: two at once for a moment, inside the four
the vendor's boards are reported to take. A picture is fetched once per print and kept for
the session.

The camera is the gap. Showing video means an RTSP client and an H.264 decoder: a C library,
or the platform's own decoders and an implementation per platform. Each carries licensing or
upkeep this ADR does not take on. A later decision does, and this ADR's camera clause is the
one it supersedes.

## Alternatives considered

### Each card holding its commands on the watch's connection

One socket per board. The watch thread would then also run the user's commands, and a
slow history read would stall the state reports behind it.

### Fetching every picture through the watch

The same objection, on a larger scale: a 480 kB bitmap read in line would stop the reports.

### Leaving the running jobs at the top

No change. The stage would keep two places that each say what is happening, and the top one
sat under the view cube.

### The option that won, and what it costs

More threads and short-lived connections, a card that now carries most of a printer's
panel, and a camera that is only an address until video is decided.
