# The plate document

One plate, edited one way by the window, the command line and `encrust-mcp` (ADR 0466).

## Crates

```
core-edit      Edit, ObjectId: what is asked, as data
core-document  Document, apply: what is done
```

`core-edit` names its settings by the types `.encrust` stores; it holds no algorithm.
`core-document` holds `Document` and every case of `apply`, moved down from the front ends.

## Document

`Document` is the plate as `.encrust` holds it: the manifest and each object's meshes, in the
same order, each object with an `ObjectId`. `from_project` gives an id to each object that has
none, after the highest present; `to_project` writes them. An id is never reused within a
document, so an edit naming a deleted object fails rather than landing on another.

## Edits

| Group | Variants |
|---|---|
| placement | `Move`, `Rotate`, `Scale`, `Mirror`, `Orient` |
| plate contents | `Delete`, `Duplicate`, `Arrange` |
| repair | `Repair`, `Mend`, `Relief` |
| shaping | `Combine`, `Label`, `Cut`, `Split`, `PartOff` |
| hollow and drains | `Hollow`, `Drain`, `Channel`, `Plugs` |
| supports | `Supports`, `MergeSupports` |

Points and positions are plate millimetres. An edit names the objects it acts on; one that
adds objects answers their ids and what each is (`Applied`), so a front end can name or select
them. Print settings, printer and resin are a revision's, not edits.

## apply

`apply(&mut Document, &Edit, Cancel)` changes the document or leaves it as it was: an error
never leaves half an edit. A caller that cannot cancel passes `Cancel::never()`.

Until the window's `Scene` is a `Document`, a front end runs an edit's steps rather than
`apply`: the window's Cut and `--cut` call `cut` and `pieces`, its Split and `--split` call
`split` and `part_off_at`, the same calls `apply` runs.

`Cut { object, axis, at_mm, keep, joints, lay_flat }` cuts the object's printed mesh, its
drain holes taken out first, at `at_mm` along `axis` on the plate. The kept halves replace it
with no supports or cavity of their own; with `lay_flat` each lies on its cut face, except a
lower half carrying pins or a key; loose dowels stand in a row beside the lower half. A plane
that misses the object is `ApplyError::Missed`. The window runs the two steps on a worker,
`--cut` on a mesh it has not made an object yet; `--cut` always lays flat.

`Move { object, by_mm }` adds to the translation. `Rotate { object, degrees }` sets the turn
(X, then Y, then Z of the plate, as `--rotate`) and `Scale { object, factors }` sets the
scale, a mirror kept, each about the object's centre of mass and then standing it on the plate;
a factor not finite or above zero is `ApplyError::BadScale`. None of them adds or removes an
object (`Report::Placed`). A placement that moves an object drops its grown trees to grow again
where it stands and carries its frozen ones there as the window does (`supports::stand_at`). The window sets the transform at once through the same `placement`
functions.

`Orient { object, goal, max_tilt_deg, clean_surface, pick, plate_mm, low_mm, printer, resin, keep_clear_mm,
keep_clear_radius_mm, keep_clear_seen_deg, lift_mm }` runs `--orient`'s search on the
object as it stands (outer, shell, cuts, drain vents, what shows), judged on the plate's active
support group and its best few printed on the shipped catalogue's `printer` and `resin`, the standard
`elegoo-saturn-4-ultra` and the stock resin numbers where unset (`printer_profiles::standard_machine`; an id
the catalogue lacks is `ApplyError::NoPrinter` or `NoResin`, one it cannot read `BadCatalogue`), and stands the `pick`th pose, from 0
(`Report::Oriented { rotate_deg }`): turned keeping the middle of its box over the same spot, slid onto the
usable area `plate_mm` across from `low_mm` where it hangs off (none where unset), dropped onto the plate and lifted
by `lift_mm`, else the profile's lift, where the pose is not set on it (`placement::stood_turned`). Past the last pose is `ApplyError::NoChoice`. `goal`
is `auto`, `fewest-supports`, `least-peel` or `shortest-print` (`OrientAim`, the one table of
the words, which a recipe's orient step reads too). A recipe's orient step is this edit
(`core-recipe`'s `edits_of`), its `pick` the first. The window
runs its Orient tool's search on the object, on the printer chosen in the window or the standard one,
and stands the `pick`th ranked pose, offered or not, by the same `stood_turned`. With no support group
the stock profile's plan stands in. MCP asks a search pose as this edit, keeping the `Rotate` handover where
a printer file is set. `keep_clear_mm` are places no support should touch, ranked first by the trial
(`kept_clear`): plate mm where the object stands, or, with `keep_clear_seen_deg`, as it stood turned so
and centred on the plate, as `--orient-keep-clear` reads them; `KEEP_CLEAR_RADIUS_MM` where no reach is
given. The window's link passes them to its Orient tool. `Report::Oriented` lists every pose ranked.
The search's share goes to a `Cancel` that reports, as `Step::Searching`, which the command line draws as a bar.

`Split { object, smallest_mm3 }` and `PartOff { object, at_mm }` run on the object's printed
mesh placed on the plate and replace it with `Role::Part(n)` pieces, counted from one: a split
by `core_volume::parts` (the default smallest part where unset), a part-off by
`core_volume::part_off` into the part and the rest. A split that finds one body and no scrap
joined or dropped is a no-op: the object keeps its id and `Report::Split` says so. Errors map
as `NoNeck | NotOnModel` to `ApplyError::NoNeck`, a stopped run to `Cancelled`, any other to
`Failed`. `apply` takes a `core_volume::Cancel` (ADR 0466's `Run`).

`Delete { object }` removes the object and the plugs laid for it. `Duplicate { object,
offset_mm }` puts a copy with all its state and meshes right after it, under a new id, a plug
of nothing, its translation moved by the offset (`Role::Copy`). `Mirror { object, axis }`
negates the scale on that axis of the object's own frame and moves it back so its bounds'
centre stays. `plug_of` is an index among the objects, so `Document` renumbers it on every
removal and insertion: a plug of a removed object names nothing.

`Combine { objects, op, toward }` replaces the objects by one `Role::Combined` piece at the
first's place, made by `core_document::combined` on their source meshes placed on the plate: the
exact `combine_exact_toward` where `toward` is set, else `core_volume::combine` (sampled where an
object is not a closed solid, `Report::Combined { voxel_mm }`). Fewer than two distinct objects
is `ApplyError::TooFew`, a missing one `NoObject`, an empty result `NothingLeft`, an unreadable
first `NotSolid`.

`Label { object, text, at_mm, height_mm, depth_mm, raised, follow_surface, angle_deg, font }`
writes `text` on the surface of the object nearest plate point `at_mm`, the object's source
placed on the plate and stamped by `combined` toward the face's normal; the result replaces it
where it stands (`Role::Kept`, `Report::Labelled`). `height_mm` and `depth_mm` unset are core-text's
`LETTER_HEIGHT_MM` and `LABEL_DEPTH_MM`; `raised` defaults to true, `follow_surface` and `angle_deg`
to flat and 0. `font` is a file, the bundled IBM Plex Sans where unset. `label_prisms` lays the
letters (the window runs it, then the stamp on a job); `labelled` is both. Errors: `BlankLabel`
(no outline in the text), `NoSurface` (no face near `at_mm`), `BadFont { path, reason }`, the
stamp's own `NothingLeft`, `NotSolid` and `Failed`.

`Repair { object, method, seam_tolerance_mm, smallest_shell_mm2 }` runs `core_document::repaired`
(`core_volume::repair` at `repair_settings`, the window's lengths where unset) on the object's source mesh and `Mend { object, flip_faces, drop_faces,
drop_shells, bridge }` runs `mended`, which `--flip-faces` and its kin call too: turns and bridges
first, drops last. Both end in `Document::reshape`, which replaces the source and takes off what
was built on the old surface (supports, cavity, painted regions; plugs stay, as the window keeps
them), but a repair that would leave the model worse and wraps nothing keeps it as it came, as
the window's does; the object keeps its id (`Report::Repaired`, `Report::Mended`). Errors: a face past the
last `NoSuchFace` (from 0), a shell `NoSuchShell` (from 1), a bridge the mesh refuses `Failed`,
`Cancelled`. The window runs a repair as its Repair tool's job and a mend as one job of the Faces mode.

`Relief { object, depth_mm, image, tile_mm, angle_deg, invert, symmetric, smoothing_px,
hold_bottom_deg, hold_top_deg, precision }` presses a picture tiled over the object, the
options defaulting as `--relief`'s flags do; `relieved` is what the CLI's `--relief-image` runs
too. core-document reads the picture through core-mesh-io's `Texture::decode`, its one reader.
The press is at the size the object prints at, so its scale and stretch go into the mesh, which
`Document::reshape` replaces (`Report::Relieved`); the faces painted for relief limit it where
any are. A picture that does not read is `BadPicture { path, reason }`, else `Cancelled` or
`Failed`. A hollow model keeps its blockers, holes and channels, carried onto the pressed surface, and is
hollowed again by the settings its shell was built to (`rehollowed`), as the window's Relief tool does, before anything on the object changes. A file's own texture is not an edit: a `.encrust` keeps none. The window's Relief tool
runs the edit as its own job, `press_picture` being the call both make.

`Hollow { object, settings: HollowState, keep_solid, keep_solid_reach_mm }` hollows the object by the
numbers of the window's Hollow panel, which it keeps as its own (`own_hollow`), the wall kept solid
along each `keep_solid` run of plate points as Keep solid clicks lay it (`blockers_along`: a ball at the
surface nearest the first, rods on; fitted to the model without a reach), the blockers kept with the
object: `settings_of` turns them into the run's
settings (an open floor faces the plate), the object's blockers and channels laid over them as
`ModelHollow::asking` does. `shelled` is `cavity_field` then `carve` on the source mesh under
`Transform::unturned`; the shell goes into `ModelMeshes::shell` and `BuiltCavity::of` into
`ObjectHollowState::built`; supports stay (`Report::Hollowed`). A wall that leaves no cavity is
`NoRoom`, else `Cancelled` or `Failed`. The window runs it as the Hollow tool's job on that object
alone; its field cache stays the window's.

`Drain { object, at_mm, size, kind }` takes each plate point into the object's own space and
drills `hole_near` its source mesh (the CLI's `--drain-at` call), adding it to the object's
`ModelHollow`; the shell is not rebuilt, the holes are cut at bake (`Report::Drained`). No
surface near a point is `NoSurface`, a point nearest an edge or corner `OffAFace`. The window
drills at once on its own `ModelHollow`. A `kind` left out is `AtDepth`, which a click and a
recipe's points drill: it stops at its depth, opening the cavity only if that reaches it.

`Drain` with `auto` then runs the engine's search (`drains_proven`, the one call the CLI's
`--drain-at auto` and the window's search job make) on the object as it stands, adds the holes it
proves, and a vent per pocket where `vent`; an object with no shell is `NoCavity`, a stop is
`Cancelled`; `Report::Drained::trapped` counts the pockets left. The window starts its own
search job on that object and the edit is done when it lands.

A `kind` of `Support` drills the raft the object stands on nearest each point (`--drain-support-at`),
kept with its holes; one standing on no raft is `NoRaft`. `Report::Drained::searched` is what the search
found and chose, which `--json` lists.

`Channel { object, points_mm, diameter_mm }` takes the points into the object's own space and
adds one channel to its `ModelHollow`, as `--channel` lays it (`Report::Channelled`).

`Plugs { object, gap_mm }` cuts a plug for each of the object's holes as they are cut
(`plugs_cut`, the CLI's `--drain-plugs` call), lays them in a row past its right-hand side as new
objects named by `Role::Plug(n)` and marked `plug_of` it, in place of those laid before, which
`removed` names (`Report::Plugged`). They stand in a row (`laid_beside`) where the window's tool
moves each to the nearest free spot (`nearest_free`). `gap_mm` is the final gap in plate mm,
no resin drift added or taken off here; the window stores the gap plus its resin's drift, which
its tool takes off again, and `edits_of` takes the drift off the default (ADR 0471). The window
starts its plugs job. The search reads the analysis step (ADR 0471).

`Supports { object, profile, at_mm, only_at, cavity, shown_at, clean_surface }` stands supports under one
object. `profile` is the plate's group of that name, else a shipped preset
(`SupportProfile::shipped`), the active group where unset; neither is `NoProfile`. A preset becomes
a group of the plate where none carries its name; the group named becomes the active one (as the
panel takes a profile up), and the contacts go into it. A profile of a user's folder reaches an
edit as a group its front end put on the plate first. `auto` sizes each need by what it holds, as
`SupportPlan::sized` does against the shipped catalogue (no user folder), by the plate's own
`SupportState::sizing` where it keeps one, else the catalogue's first; each shape that holds a
need is the plate's group of its name or a new one, and a shape the plate has a group for is built to
that group's profile; the `at_mm` points go into the group in hand.
`cavity` runs `needs_on` again on the inside of the wall, with the outside's contacts held
(the window's inside pass); `shown_at` strokes are the plan's, read by `shown_on` where the object
has no painted shown region. The painted shown region is read only with `clean_surface` or
strokes, as the window's Clean surface (ADR 0425); the front end sets the flag from its toggle. The object's `ModelSupports` is restored
by `supports_of`; `needs_on` (core-engine's one needs step) reads its printed surface as placed, its
blocked and shown regions, its contacts, the `at_mm` points among them, as seeds and, on a hollow one, the outside of the wall; the
`at_mm` points (the surface nearest each) stand before the run and each need is added after, the trees grown by `Growth::grow` and
kept as `ObjectSupportState::grown` (`Report::Supported`). `only_at` places the points alone. The
run reads the model at the analysis step (ADR 0471). The window starts its automatic
run on that object alone, adding to what stands.

`Report::Supported` gives every contact, plate mm, the tips standing and each contact no tree carries
with why, which the command line's report says. `MergeSupports { object, near_mm }` joins the supports
whose contacts lie within `MERGE_NAMING_MM` of each point into one tree, as the window's Merge into one
joins those picked, the rest grown again beside it; a point near no contact is `NoSupportNear`, points
naming one support `OneSupportToMerge`, a tree the model or the plate refuses `MergeRefused` with why.

`Arrange { low_mm, plate_mm, clearance_mm }` spreads every visible object of the active plate over
the usable area (the plate less its inset) `plate_mm` across from its corner `low_mm`, zero where
unset; the front end that knows the machine works both out (ADR 0471). Each object is one footprint with its supports and raft as `supports_of`
meshes them (`Standing`, `pack`; the window's Arrange packs through the same `pack`). An object no
slide fits is first turned the least that does (`turn_to_fit`, ADR 0418); the margin is
`ArrangeSettings`' default. The supports regrow where the object now stands, so the edit moves
only its transform. `Report::Arranged` names the objects left out; one with no surface is
`NoFootprint`.

## Undo

The window keeps snapshots of its scene (ADR 0086); a document applied is one entry. The
window's `version` counts recorded entries, undo and redo, for the link
(`window-link.md`).
