# 0419. Check the plate quickly in the background, and let the full check refine it

- **Status:** Proposed
- **Date:** 2026-11-10

## Context

Check reads the layers only once its tool or Preview is open. On a hollowed plate with
supports, the stack takes 4–5 s on eight cores, so the user waits for every answer, and the
dot on the Check tool does not show layer issues until Check has been opened.

Most of the cost is in rasterising and curing each layer, and that grows with the rows and
spans of the panel. On a panel with a 0.018 mm pitch, the issues Check names are larger than a
pixel: a cup is at least 1 cm², a thin section 0.15 mm across, a neck is judged by its
stress. Only an island near the smallest one named, 0.01 mm², is a few pixels wide. A model
can never print a detail that small anyway: light spreads past the pixel, and the peel breaks
such a detail off.

## Decision

- **A quick check reads the panel in squares of 2 by 2 pixels.** The panel has the same
  size in millimetres, half the pixels across and down, and twice the pitch.
  `RasterSettings::binned` describes it, `Run::measure_binned` measures it, and
  `QUICK_CHECK_BIN` is the size of the square. Risk positions come out in the same panel
  millimetres. The command line reaches it with `encrust estimate --quick-check`.
- **The window runs the quick check in the background.** It starts once the plate has not
  changed for 1.5 s. A change stops the check in progress. It runs in its own rayon pool on
  half the cores, so the window and a full check keep the rest. Its result colours the dot on
  the Check tool and is shown in Check, marked preliminary, until the full check lands.
- **Opening Check runs the full check, as before, and its result replaces the quick one.**
  Check rebuilds its cards from the issues every frame, so nothing has to be matched: an
  issue both checks found keeps its card and takes the full check's figures, one the full
  check adds appears, and one only the quick check found goes, since the full check read
  every pixel the quick one did. Until the full check lands, the quick issues are shown with
  a note under them and their fix buttons are off; Fix all and recipes run their own full
  check.

## Consequences

- On the bench, a quick check takes 158 ms where the full one takes 233 ms (20 spheres), and
  12 ms where it takes 16 ms (one sphere). Cutting and folding do not shrink with the pixels,
  so the gain is 1.3–1.5×, not 4×. Opening Check is quicker for the user mainly because the
  quick answer is already there.
- The background check uses CPU and battery after each finished change: about one quick check,
  a few seconds on a heavy plate, on half the cores.
- A quick check can miss an island near 0.01 mm². It can also name a false island where a
  bridge one or two pixels wide disappears in a square. The full check corrects both.
- On a panel an odd number of pixels across, the quick panel is one pixel wider, so its
  issues are placed up to one pixel off.
- Reopen this if the quick check's misses show up in prints, or if a 3 by 3 square proves
  just as good on the bench.

## Alternatives considered

### Full checks in the background

They need no merging and are never wrong, but they cost 1.5× more on every change, and
running them on the whole pool slows the window.

### Keep issues only the quick check found

A union of both checks never drops anything, but it keeps the false islands that a square
creates over a thin bridge. The full check has read every pixel, so its silence is an answer.

### Squares of 2 by 2, and what they cost

The quick check is wrong near the smallest island, and the window keeps two results for one
plate. The full check is still needed before anything is fixed or written.
