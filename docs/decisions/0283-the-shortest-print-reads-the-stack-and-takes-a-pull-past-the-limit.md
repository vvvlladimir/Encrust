# 0283. The shortest print reads the stack, and takes a pull past the limit

- **Status:** Accepted
- **Date:** 2026-10-15

## Context

On an MSLA machine a layer takes as long whatever its area: the whole screen cures at
once, and a tilting vat such as the Saturn 4 Ultra's runs one fixed cycle a layer (ADR
0142). For one profile the print time is a constant plus a constant per layer, so the
order of poses by time is their order by layer count, and that is the height of the stack:
the model and the lift that stands it on supports.

The shortest print read the model's height without the lift in the search and the layer
count of the coarse trial stack once printed: two units, neither the stack. A pose set on
the plate, 5 mm lower and 100 layers shorter at 0.05 mm, read as tall as a lifted one in
the search. And like every goal but the fewest supports it failed any pose pulling past
the 30 N limit, a scale known to about a factor of three and checked on one print that
tore at 67 N: a solid 100 mm figure, 2000 layers as it stood, was turned to 2197 layers
because every pose on its side read 40–44 N, the window saying nothing of why.

## Decision

- **The shortest print reads the stack, millimetres.** `Score::stack_mm` is the height and
  the lift under a pose on supports; `Printout::stack_mm` is the trial's layers times its
  layer. The search reads it to 0.1 mm, the proof to two of a trial's thickest layers,
  0.3 mm. The height term of every goal's total reads the stack too.
- **A pull past the limit fails no pose for the shortest print.** `Readings` carry cups
  (`suction`) and tearing pulls (`tearing`) apart, and a `Policy` says whether it weighs
  the second (`weighs_tearing`); every goal but the shortest print does. Cups, islands and
  contacts with no room still count. The search weighs no peel in its total.
- **Its band is a fiftieth of the shortest and a millimetre**: some four minutes on a
  100 mm figure, two on a flat part. Within it the fewest left hanging, then the shortest
  stack; support is not weighed. A turn has to leave the band or win 5 %.
- `orient-lab` reads the shortest print in layers of the full print, with its own noise of
  five layers, and does not count a tear against it.

## Consequences

The goal is what it says: the fewest layers that hold and seal no cup. A pose set on the
plate is as short to the search as it prints, and the search and the proof read one
quantity. A slab lies flat, a figure on its side, whatever they pull, and the window's
peel warning is the user's to weigh; a print that tears where this goal chose is the
signal to give it a limit again, measured on the printer.

The goal takes whatever support the shortest pose needs. A person who wants a balance
picks another goal; weighing several is a separate piece of work.

## Alternatives considered

### Support within the band

A first version let the fewest supports decide between poses within the band. With a band
of a twentieth it turned figures upside down for a millilitre at ten minutes' cost; with a
fiftieth and support alike within a quarter it behaved, but answered a question the goal
does not ask, and was dropped on review.

### A tearing pull as a risk rather than a failure

Counted like a hanging contact, a tear would still lose to a pose that holds within the
band. It keeps a number the limit's own accuracy does not support, so the goal ignores the
pull outright.

### Time from the printer profile

`PrintJob::print_time_s` gives seconds. It needs the profile in the search, which has
none, and for one profile ranks exactly as the stack does.

### The stack, and what it costs

A pose that tears on a real printer is now chosen where it is the shortest; and poses
differing by under a millimetre on a flat part are a tie however many layers that is.
