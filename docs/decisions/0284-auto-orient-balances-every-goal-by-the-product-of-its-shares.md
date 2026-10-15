# 0284. Auto-orient balances every goal by the product of its shares, and the clean top goes

- **Status:** Accepted
- **Date:** 2026-10-15

## Context

Auto-orient asks for one goal: the fewest supports, the least peel, the shortest print or
a clean top. Most users want none of them alone. They want a pose that is not bad at any
of them, and they cannot say how many millilitres a minute is worth. Each goal already
carries every goal's reading of every candidate, in the search and once printed (ADR
0281), so the readings are there. What is missing is a rule that weighs millilitres,
newtons and millimetres against each other.

On the `orient-lab` corpus, 14 of 21 models have a pose within every goal's band at once.
On the rest the goals really do pull apart: a 98 mm disc with pockets reads 0.79 ml and
103 mm on its edge, or 6.19 ml and 21 mm face down. Dividing each reading by its goal's
band, which is how far it can be off and still read as good, lets the narrowest band
decide. The stack's band is a fiftieth, and the disc goes face down for eight times the
support. A band says when two readings are alike. It is not a price.

The clean top read the faces that look up in the file. That stands in poorly for what a
person sees, so on most models it chose as the model was drawn, whatever that cost the
other goals.

## Decision

- **`OrientPreset` is what a caller asks for**: `Goal(OrientGoal)` or `Auto`.
  `OrientSettings::preset` replaces `goal`. `OrientGoal` stays the list of measured goals,
  which every reading array is indexed by.
- **Auto weighs each goal's reading as a share of the best a sound pose reads, and takes
  the pose with the smallest product of shares** (the sum of their logarithms). A floor is
  added under each reading: ten struts of support, nothing past the 10 N enough pull,
  five noises of stack. Fail-the-print and hanging come first, with cups and tearing pulls
  counted as fatal as the least peel counts them. Next come poses that sit inside every
  goal's band at once, then poses no sound pose beats on every goal. Ties go to the
  smallest worst share. A turn has to cost a twentieth less, and more than its noise. A
  model whose current pose is inside every band does not turn.
- **Auto's search follows its answer with each goal's own best.** That way the proof
  prints every end of the trade within the same four to sixteen poses.
- **The search weighs a candidate for auto by the mean of every goal's weights.**
- **`Ruling::Alternative` and `Ruling::Dominated`** say which finalists auto would offer
  as trades and which another pose beats on every goal. `Ruling::is_offered` names the
  poses worth showing beside the chosen one, for any preset. `Oriented` carries its
  ruling. The window offers those poses one at a time beside the first, and shows what
  each costs where a proof printed it.
- **The clean top is removed** from `OrientGoal`, the search's weights, the shortlist,
  the proof, the window and `--orient-goal`. `Printout::top_contacts` stays as a reading
  `orient-lab` shows.
- `orient-lab` judges auto by its own rule: sound, inside every band where some pose is,
  beaten on every goal by no sound pose, and no goal past three times its best.

## Consequences

A goal added to `OrientGoal::ALL` is weighed by auto with no change to the balance: its
policy's band, enough and floor are all auto needs. Because the product does not depend
on units, a goal can be read in any unit that is proportional to its cost.

On a real trade such as the pocketed disc, auto takes the side that gives up the smaller
share. That is a judgement, and the user can step through the other trades in the window.
The floors are new numbers. On the corpus, moving the support floor from 0.2 to 1 ml and
the stack floor from 5 to 30 mm changes no pick, but at 2 ml and 20 mm the disc goes face
down. A user who reports that auto keeps picking the side they step away from is the
signal to revisit the floors or offer a slider.

Changing `OrientSettings::goal` to `preset` and removing `clean-top` from the command line
breaks scripts that named it.

## Alternatives considered

### Dividing by each goal's band

This was the first plan. It is unitless and reuses numbers each goal already has, but as
the disc shows, it lets the narrowest band decide.

### Ideal-to-nadir normalisation, TOPSIS

Scaling each goal between its best and its worst candidate is the textbook route. The
worst candidate is often an absurd pose that the search happened to cut, and the pick
moves with it.

### Weights from the spread of each goal (entropy, CRITIC), or learnt

With two to eight sound poses, the spread comes from one or two of them, and entropy
weighting gives the most weight to the goal with the widest spread, absurd pose included.
Learning weights needs a right answer, and on the contested models there is none, only
taste.

### A knee of the Pareto front

The knee is the usual choice when no preference is given, but it needs a dense front. Ours
has two or three poses.

### The product of shares, and what it costs

It is a choice, not a fact. It is the balance that has no units, and the user's taste
shows only through the poses offered beside it. Its floors are tuned on one corpus. A
reading near zero without a floor would make any turn away from it infinitely costly.
