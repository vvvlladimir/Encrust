# What Check's buttons do

Each button runs a call the command line runs too: a recipe's `check` and `arrange` settings,
`docs/design/recipes.md`. Why these answers: ADR 0415. Fix all takes the first answer of every
kind but peel and thin as `assets/recipes/fix-issues.toml` lays them out: ADR 0416.

| Issue | Buttons | Call |
|---|---|---|
| Past the build volume, in the inset, meeting | Move to free space | `core_plate::turned_onto`, `move_clear`, then `arrange` where no clear place is left |
| Larger than the build volume | Scale to fit | the Position tool's |
| Meeting | Combine | opens Combine on the pair listed first |
| Hollow with no drain hole | Drill drain holes | Drain's Auto on those models |
| Suction cup | Drill them, Auto-orient | `core_engine::cup_vents`, the `auto` preset |
| Island | Support them, Remove them | `core_engine::island_holds`, each support in the lightest automatic shape `held_by_shape` finds bears the island's pull |
| Weak neck | Support them, Auto-orient | `core_engine::lever_holds` on the lightest automatic shape `held_by_shape` finds holds the neck, the `auto` preset |
| Hard peel | Reduce peel, Lift settings | the least-peel preset; opens Print settings where the printer leaves the lift to the file |
| Thin section | none | no operation thickens a model |

## Where a vent goes

A ray goes straight down from the cup at the height it is widest to its floor: a face looking
up, or the plate where it meets none. The vent starts at the cup's lowest point, a quarter of
the hole's width over the floor, so half the hole sinks into it and leaves no cup under its
rim. From there 17 rays go out, straight down, down at 45° and level, eight ways round each.
Each one enters a wall and leaves it once it has left every shell it entered, at most
`MOST_CROSSINGS` faces on. The thinnest wall wins, up to `LONGEST_VENT_MM` (15 mm). The hole is
drilled along that ray from where it leaves, and goes on `VENT_PAST_WALL_MM` into the cup
(ADR 0417).

A level vent out of a cup on the plate runs along it as a notch, as a channel in a base is
cut by hand. A vent
going down is refused where the plate lies under its rim, which would seal it again.

There is no vent where the first ray meets a face looking down: the centre of a ring-shaped
cup lies in the material. Nor where every wall is thicker than the limit; when the cup's floor
is the plate, or stands on it, Check says to lift the model onto supports or turn it.

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
one model into another's concave side, which packing the plate does.

A model no slide fits is first turned about the vertical by the least turn that does; one with
no clear place left has the whole plate packed instead (ADR 0418).

A model the plate's 2 mm margin leaves no room for is placed flush with the edge rather than
left where it stands, by Move to free space and Arrange alike; the inset is never crossed
(ADR 0415).
