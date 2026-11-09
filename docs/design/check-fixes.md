# What Check's buttons do

Each button runs a call the command line runs too: a recipe's `check` and `arrange` settings,
`docs/design/recipes.md`. Why these answers: ADR 0415. Fix all takes the first answer of every
kind but peel and thin as `assets/recipes/fix-issues.toml` lays them out: ADR 0416.

| Issue | Buttons | Call |
|---|---|---|
| Past the build volume, in the inset, meeting | Move clear, Arrange all | `core_plate::move_clear`, `arrange` |
| Larger than the build volume | Fit the volume | the Position tool's |
| Meeting | Combine | opens Combine on the pair listed first |
| Hollow with no drain hole | Drill drain holes | Drain's Auto on those models |
| Suction cup | Drill them, Auto-orient | `core_engine::cup_vents`, the `auto` preset |
| Weak neck | Support them, Auto-orient | `core_engine::lever_holds`, the `auto` preset |
| Hard peel | Least peel, Lift settings | the least-peel preset; opens Print settings where the printer leaves the lift to the file |
| Thin section | none | no operation thickens a model |

## Where a vent goes

A ray goes straight down from the cup at the height it is widest. It must meet a face looking
up: the cup's floor. A second ray from just under that face finds where the floor ends. The
hole opens there, drills up through the floor and goes on `VENT_PAST_FLOOR_MM` into the cup.

There is no vent where the first ray meets no such face, which is a ring-shaped cup whose
centre lies in solid material, or where the floor's underside is within a layer of the plate,
since the plate would seal the hole again.

## Where a neck's support goes

Each support joins the neck as the analysis reads it: its tip's area adds to the neck's, and
the neck's centre moves towards the piece's. `core_analysis::holds_for_lever` counts the
fewest supports that bring the stress under 0.8 of `NECK_LIMIT_MPA`, at most
`MOST_NECK_HOLDS` (16); the margin covers a tip curing a little under its drawn size. The
supports are drawn at least 0.6 mm across, tip and small pillar alike: a thinner one adds
next to nothing to a neck, and Check's re-measure would find it weak again.

Rays go straight down from the middle of the piece and from rings around it at its radius of
gyration and half of it, at the height of its layer. A face looking down is the piece's
underside and takes a support, one tip's width from the others; a face looking up is outside
the piece. A neck with no underside clear of the plate is left to the hand.

## Moving only what is in the way

Every model past the usable area moves, and of each meeting pair the smaller unless the other
is already moving. They go largest first to the nearest place clear of the rest by
`nearest_free`, on their bounding boxes: exact, without a grid, at the cost of never nesting
one model into another's concave side, which Arrange all does.

A model the plate's 2 mm margin leaves no room for is placed flush with the edge rather than
left where it stands, by Move clear and Arrange alike; the inset is never crossed (ADR 0415).
