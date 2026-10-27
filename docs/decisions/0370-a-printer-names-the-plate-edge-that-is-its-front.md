# 0370. A printer names the plate edge that is its front

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

The vendor slicer's profile for each machine states this as `direction_type`: `Front`,
`Left` or `Right`. Of 194 machines, 36 say `Left` or `Right`, all but one with a portrait
panel.

## Decision

`PrinterProfile` carries `front_edge`: `min_y` (the default, and left unwritten), `min_x` or
`max_x`, transcribed from the vendor's `direction_type` (`Front`, `Left`, `Right`). It is for
the viewport only: the plate keeps panel axes, so raster, files and placement do not change.

The viewport turns three things about Z by `FrontEdge::turn_rad`: the word Front, the
opening and Home camera, and the view cube's faces with their names.

## Consequences

- A portrait machine opens looking at its real front, and the cube's Front clicks there.
- Plate X and Y stay the panel's, so the grid's axis marks still read the panel's X along
  the short side on these machines, as the vendor slicer shows it.
- Profiles with no vendor record (the Nova3D machines, the SL1 and SL1S)
  keep `min_y` until a machine or a vendor file says otherwise.
- The printer form does not offer the field; an own profile gets it by editing its TOML.

## Alternatives considered

### Swap the plate's axes and turn the raster on writing

The plate would read wide on these machines with X along the long side. Every writer, the
raster's mirror flags and the sliced-file reader would gain a turn, for a change the user
sees only in the viewport; and the vendor slicer keeps panel axes too.

### Guess the front from the panel's shape

A portrait panel would put the front on a long side. Three portrait machines say `Front`
(the Photon Mono SQ among them) and one landscape machine says `Right`, and the guess cannot
choose between `-X` and `+X`.

### The option that won, and what it costs

One more profile field to transcribe and keep right per machine, and the `Left`/`Right`
to `-X`/`+X` reading of the vendor's word is taken from its viewport, not from a print.
