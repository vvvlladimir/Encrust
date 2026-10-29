# 0369. A printer names the plate edge that is its front

- **Status:** Accepted
- **Date:** 2026-10-28

## Context

Plate axes are panel axes: X runs along the panel's `width_px`, so a model placed on the
plate rasterises where it stands (`crates/encrust-app/src/plate.rs`). The viewport put the
word Front, the opening camera and the view cube's Front face on the plate's `-Y` edge.

That is right for a panel mounted landscape. Many older machines — 2560 px tall portrait
panels such as the Creality LD-002H, the Elegoo Mars line, the Phrozen Shuffle — mount the
panel turned, so the machine's front is a long side, the panel's `-X` or `+X` edge. The
viewport showed it on a short side, and a user setting a part near the front of the vat put
it at the side.

The maker's settings for each machine state this as `Front`,
`Left` or `Right`. Of 194 machines, 36 say `Left` or `Right`, all but one with a portrait
panel.

## Decision

`PrinterProfile` carries `front_edge`: `min_y` (the default, and left unwritten), `min_x` or
`max_x`, transcribed from the maker's word (`Front`, `Left`, `Right`). It is for
the viewport only: the plate keeps panel axes, so raster, files and placement do not change.

The maker's `Left` is the plate's `-X` edge, `Right` its `+X`. On an X-mirrored panel other
than `.photons` a known layer is ours turned half round (ADR 0366), so there `Left` is our `max_x`
and `Right` our `min_x`; elsewhere `min_x` and `max_x`. One LD-002H part placed alike in both
slicers' windows showed it: the two files held it in opposite corners.

The viewport turns three things about Z by `FrontEdge::turn_rad`: the word Front, the
opening and Home camera, and the view cube's faces with their names.

## Consequences

- A portrait machine opens looking at its real front, and the cube's Front clicks there.
- Plate X and Y stay the panel's, so the grid's axis marks still read the panel's X along
  the short side on these machines, as the makers draw it.
- Profiles with no maker's word (the Nova3D machines, the SL1 and SL1S)
  keep `min_y` until a machine or a known file says otherwise.
- The printer form does not offer the field; an own profile gets it by editing its TOML.

## Alternatives considered

### Swap the plate's axes and turn the raster on writing

The plate would read wide on these machines with X along the long side. Every writer, the
raster's mirror flags and the sliced-file reader would gain a turn, for a change the user
sees only in the viewport; and the makers keep panel axes too.

### Guess the front from the panel's shape

A portrait panel would put the front on a long side. Three portrait machines say `Front`
(the Photon Mono SQ among them) and one landscape machine says `Right`, and the guess cannot
choose between `-X` and `+X`.

### The option that won, and what it costs

One more profile field to transcribe and keep right per machine, read through the file
layout as well as the maker's word. On those X-mirrored machines the grid's X runs towards
the front where the maker's runs away from it.
