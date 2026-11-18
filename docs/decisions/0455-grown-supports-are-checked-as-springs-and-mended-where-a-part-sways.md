# 0455. Check grown supports as springs, and mend where a part sways, a trunk buckles or a tip tears

- **Status:** Accepted
- **Date:** 2026-11-18

## Context

`TipLoads` shares each layer's pull among the tips under a part as one rigid section (ADR 0422):
a 76 mm trunk counts as much as a 5 mm one, the part does not bend, and weight is ignored.
On a dragon, Auto held each wing — a membrane about 0.45 mm thick, 38 mm across, apart from the
body for 50 mm of rise — at its lower edge on 1.0 mm trunks 30 to 76 mm tall, and left 5 mm of
it with no contact. The rigid read gave its tips ×30 to spare; a print of such a wing tears.
Nothing in placement or the bench measures how a support gives, and strengthening runs on
contacts before any tree is grown.

No modulus of green resin was measured. A data sheet gives Anycubic ABS-Like Pro 2 a cured
flexural modulus of 1.0 to 1.8 GPa; published green values sit at 0.57 to 0.85 of cured.

## Decision

- **Each tip stands on a spring** read off its grown tree by the unit-load method
  (`core_supports::gives`): along z, across and turning, every strut's bending, twist and
  stretch summed from the tip to where the tree stands, a strut shared by `n` tips counted `n`
  times. Below a trunk's highest brace only its stretch counts. The result is compliance times
  the modulus, so the modulus stays in `core-analysis`.
- **A part is a rigid body on those springs** (`carry/frame.rs`): six unknowns solved per layer
  for the pull round its peel ellipse and the part's weight, each tip carrying what its own
  spring is moved by. Tension and the turn bend the neck as before; the lateral force is shear.
  A part whose tips all have springs is read so; any other keeps the rigid section.
- **A support standing on the model stands on the part under its foot**, found as the stack
  climbs past it. On the part it holds, it ties that part to itself and holds none of it. On
  another part, its spring is in series with how that part gives there, read on that part's
  own springs; the force it carries is passed down to that part, whose tips are read again
  with it, one level deep. Its foot meets the model by one contact (ADR 0124), which every
  tip of its tree loads: a tip's stress is the worse of its own neck and that contact.
- **A part bends above its highest tip** (`carry/bend.rs`): each layer's section is a beam's,
  summed in four running totals reset where a tip seats or parts join, so no section is kept.
- **Weight** of the part grown so far, 1.15 g/cm³ in air with no lift from the vat, pulls with
  each layer, on both reads.
- **Three limits:** the layer moves across by at most `SHIFT_LIMIT_MM` 0.1 mm; a trunk takes the
  film's push on the way down, as hard as its pull, with `WANTED_MARGIN` against Euler buckling
  over twice its free length; a tip keeps `WANTED_MARGIN` against `NECK_LIMIT_MPA`.
- **`GREEN_MODULUS_MPA` is 600**, the data sheet's lowest times the lowest green share.
- **`core_engine::firm_up` runs after strengthening**, in Generate, `--supports` and the bench:
  it grows the trees, reads the springs and adds, at most three rounds, one contact on the
  highest underside within a spacing under each place a part swayed, or strengthening's
  heavier tip or neighbour where a tip tears or a trunk buckles. The trees it grows are thrown
  away; the window grows its own as before.
- **Auto Medium's pillars are 1.3 mm and Auto Heavy's 1.6 mm, Heavy's trunks up to 3.0 mm**,
  where 0424 stood every size on 1.0 mm: a column gives as its radius to the fourth, so a
  heavier size now holds stiffer, not only wider at its tip.

## Consequences

On the dragon, Auto puts 1591 contacts instead of 1549 and 0.6 ml more resin; 26 of the 42 land
on the wings, among them 5 mm of one that had none. Generating takes about twice as long, the
trees being grown up to three more times; an orientation's proof, which stands supports as the
print does, slows alike. A part held at its foot alone now gets contacts up its length. Sizing
now moves contacts too: a lighter shape sways where a heavier one stands.

The read moves with the modulus and the shift limit, neither measured. A part rocking on its
first tips, or one held as its own sections bend, are what the limit reads; a print of the
dragon's wings, a sway seen where the read called still, or Generate felt too slow reopens it.
Braces are read as holding a trunk still across. On the dragon a support standing on its body
reads as standing still, the body being held by some five hundred tips, and once a wing joins
the body its supports from the body hold nothing; neither changed a contact.

## Alternatives considered

### A frame of every strut and brace, solved whole

Exact for loops, but a sparse solver is a new dependency, and the condensation to each tip
is the work this record does in closed form for a tree.

### Measure the modulus on a coupon first

Asked for, and set aside: the read ranks parts the same at any modulus, and a coupon is a
print. The data sheet's lowest errs towards more supports.

### Check before the trees are grown

Cheaper, on straight columns guessed under each contact; but merging and bracing are what
make a support stiff or not, and a guess would mend what stands and miss what does not.

### The option that won, and what it costs

Supports are mended on numbers no print has confirmed, and Generate grows its trees up to four
times. A stiffness-weighted share also loads a short stiff tip more than the rigid read did, so
some tips strengthening passed are strengthened again.
