### The window's text in English, the language every other catalogue translates.
### A group comment names the source file its messages are read in; see docs/design/i18n.md.

## Shared by many panels

common-already-done = Already done
common-cancel = Cancel
common-export = Export
common-settings = Settings
error-cannot-hand-over = cannot hand over { $name }
error-cannot-open = cannot open { $path }
error-cannot-read = cannot read { $path }
error-cannot-write = cannot write { $path }
profiles-delete = Delete
profiles-edit = Edit
profiles-every = Every profile
profiles-shipped = A profile this build ships cannot be deleted.
status-exported = Exported { $path }
status-opened = Opened { $path }
status-saved = Saved { $name }
table-profile = Profile
unit-deg = deg
unit-degrees = °
unit-g-ml = g/ml
unit-h = h
unit-m = m
unit-mm = mm
unit-mm-min = mm/min
unit-px = px
unit-s = s
value-g = { $value } g
value-ml = { $value } ml
value-mm = { $value } mm
value-mm2 = { $value } mm2
value-s = { $value } s
value-um = { $value } µm

## panels/settings/general.rs

general-language = Language
general-language-system = As the system ({ $name })
general-language-untranslated = The window is not translated into the system's language ({ $tag }) yet, so it shows in English.
general-start = Start
general-start-page = Open on the start page
general-start-page-hint = The drop zone, the files opened lately and the resin test prints, until a model arrives. Off, a run opens straight on the plate.
general-viewport = Viewport
general-invert-zoom = Scroll down to zoom in
general-sliced-files = Sliced files
general-sliced-dir-system = The save dialog opens wherever the system last left it.
general-sliced-dir = The save dialog opens in { $dir }
general-choose-folder = Choose a folder
general-forget-folder = Forget it
general-profiles = Profiles
general-profiles-nowhere = This system has nowhere to keep your profiles.
general-profiles-dir = Your machines, resins and profiles are kept in { $dir }
general-open-folder = Open the folder

## panels/inspector/cut.rs

cut-plane = Plane
cut-select-model = Select a model to cut.
cut-height = Height
cut-offset = Offset
cut-measured-from-plate = Measured from the plate.
cut-measured-from-centre = Measured from the model's centre of mass.
cut-afterwards = Afterwards
cut-lay-flat = Lay the halves flat
cut-lay-flat-note = Each half stands on the face the cut left.
cut-pins = Alignment pins
cut-pins-note = Pegs and sockets so the halves glue back true.
cut-pin-count = Pins
cut-pin-diameter = Diameter
cut-pin-length = Length
cut-pins-sockets = Pins stand on the lower half; the sockets are { $clearance } mm wider each side and show as holes in the Drain tool.
cut-replaces = Two models replace one. Undo puts it back.
cut-cut = Cut
cut-split = Split into its parts

## cut.rs

cut-nothing-selected = nothing is selected to cut
cut-no-direction = the cut plane has no direction
cut-misses = the plane misses { $name }, so there is nothing to cut
cut-lower-name = { $name } (lower)
cut-upper-name = { $name } (upper)
cut-part-name = { $name } (part { $index })
cut-too-small-for-pin = ; the cut is too small for a pin
# Starts with a space: it follows the cut's position directly.
cut-with-pins = { " " }with { $count } pin(s)
cut-done = Cut { $name } at { $axis } { $position } mm{ $pinned }{ $mending }
cut-done-open = Cut { $name }; { $open } cut loops were open and left uncapped{ $mending }
cut-nothing-to-split = nothing is selected to split
cut-one-piece = { $name } is one piece
cut-split-done = Split { $name } into { $count } parts{ $mending }
cut-mended =
    { $count ->
        [1] ; one piece was mended to close it
       *[other] ; { $count } pieces were mended to close them
    }

## panels/inspector/cut.rs

cut-keep-both = Both
cut-keep-lower = Lower
cut-keep-upper = Upper

## workspace.rs

tool-select = Select
tool-measure = Measure
tool-hollow = Hollow
tool-drain = Drain
tool-cut = Cut
tool-relief = Relief
tool-supports = Supports
tool-paint = Paint
tool-shape = Shape
tool-check = Check
tool-export = Export
tool-print-settings = Print settings

## shortcuts.rs

action-undo = Undo
action-redo = Redo
action-select-all = Select all
action-deselect = Deselect
action-duplicate = Duplicate
action-remove = Remove from the plate
action-next-view = Next view
action-previous-view = Previous view
action-frame-view = Frame the view
action-settings = Settings
action-sheet = This sheet
action-find = Find
action-layer-on = One layer on
action-layer-back = One layer back
action-ten-layers-on = Ten layers on
action-ten-layers-back = Ten layers back
action-play = Play
action-new-plate = New plate
action-new-project = New project
action-open-project = Open a project
action-save-project = Save the project
action-save-project-as = Save the project as
action-import-model = Import a model
action-slice = Slice
shortcut-group-tools = Tools, by their place on the rail
shortcut-group-plate = The plate
shortcut-group-window = The window
shortcut-group-layers = The layers
shortcut-group-files = Files
gesture-move-focus = Move between the controls
gesture-move-focus-keys = Tab, Shift Tab
gesture-press-focus = Press the control in focus
gesture-press-focus-keys = Enter or Space
gesture-leave-focus = Leave the control in focus
gesture-leave-focus-keys = Esc
gesture-add-selection = Add to the selection
gesture-add-selection-keys = Cmd click
gesture-pick-between = Pick everything between, in the list
gesture-pick-between-keys = Shift click
gesture-orbit = Orbit
gesture-orbit-keys = Drag
gesture-pan = Pan
gesture-pan-keys = Right drag
gesture-zoom = Zoom
gesture-zoom-keys = Scroll
gesture-zoom-mask = Zoom into the mask
gesture-zoom-mask-keys = Scroll or pinch
gesture-move-mask = Move the mask
gesture-move-mask-keys = Drag
gesture-whole-mask = Show the whole mask
gesture-whole-mask-keys = Double click
status-removed-models =
    { $count ->
        [one] Removed { $count } model
       *[other] Removed { $count } models
    }

## panels/shortcuts_sheet.rs

shortcuts-title = Keyboard shortcuts
shortcuts-note = A tool takes the digit of its place on the rail. Keys are off while a field has the cursor.
shortcuts-or = or

## settings.rs

settings-page-supports = Supports
settings-page-hollow = Hollow
settings-page-drain = Drain
settings-page-general = General
settings-page-updates = Updates
settings-tab-resins = Resins
settings-tab-machine = Machine
settings-tab-network = Network
settings-new-resin = New resin
settings-new-printer = New printer
settings-new-profile = New profile
settings-copy-name = { $name } copy

## supports.rs

supports-placing-place = Place
supports-placing-fill = Fill
supports-placing-block = Block
supports-placing-erase = Erase
supports-nothing-visible = Nothing visible on the plate to hold up.
supports-nothing-visible-error = nothing visible on the plate to hold up
supports-placed =
    { $count ->
        [0] Nothing on the plate needs holding up
        [one] Placed { $count } support
       *[other] Placed { $count } supports
    }
supports-cancelled = Placement cancelled
supports-group-name = Group { $index }

## measure.rs

measure-snapped-corner = on a corner
measure-snapped-edge = on an edge
measure-snapped-free = free

## panels/inspector/supports.rs

supports-where = Where
supports-picked = Picked
supports-already-done = Already done
supports-pick-hint = Click a tip, a strut, a joint, the trunk or the foot to pick it. Drag to carry it, shift adds, alt removes.
supports-picked-help = Grow these again hands the picked supports back to the automatic run, and the next growth builds them afresh. Move to the group in hand builds them to that group's profile.
supports-stat-parts = Parts
supports-stat-supports = Supports
supports-stat-standing = Standing
supports-move-to-group = Move to the group in hand
supports-grow-these-again = Grow these again
supports-remove-these = Remove these supports
supports-grow-again = Grow them again
supports-grow = Grow the supports
supports-grow-tooltip = Cuts { $scope } to find what hangs over nothing.
supports-placing = Placing supports
supports-no-room = Some supports have no room to stand and are not printed.
supports-remove-all = Remove all
supports-remove-all-tooltip = Take every support away
supports-remove-group = Only { $name }
supports-remove-group-tooltip = Take away the { $count } of the group in hand and leave the rest

## panels/settings.rs

settings-title = Settings
settings-shortcuts = Shortcuts
settings-import = Import
settings-cannot-export = cannot export the profile
settings-cannot-delete = cannot delete the profile
settings-name = Name

## panels/settings/machines.rs

machines-title = Machine and resin
machines-search = Your machines
machines-no-match = No machine of that name.
machines-from-library = Pick yours from the library
machines-add = Add a machine
machines-from-file = From a printer profile file, or drop one here
machines-import = Import a machine
machines-remove = Remove this machine
machines-import-file = Import a machine from a file
machines-pick = Pick a machine, or add one from the library.
machines-in-hand = The plate prints on this
machines-use = Use this machine
machines-cannot-add-printer = cannot add a printer
machines-cannot-add-machine = cannot add the machine
machines-cannot-add-a-resin = cannot add a resin
machines-cannot-add-resin = cannot add the resin
machines-cannot-duplicate-resin = cannot duplicate the resin
machines-cannot-remove = cannot remove the machine
machines-cannot-take-resin-off = cannot take the resin off
machines-cannot-delete-resin = cannot delete the resin

## panels/inspector/check.rs

check-filter-all = All
check-filter-islands = Islands
check-filter-levers = Levers
check-filter-peel = Peel
check-filter-cups = Cups
check-filter-thin = Thin
check-verdict = Verdict
check-support-islands = Support every island
check-drill-cups = Drill the suction cups
check-issues = Issues
check-found =
    { $issues ->
        [one] { $issues } issue
       *[other] { $issues } issues
    } on { $layers ->
        [one] { $layers } layer
       *[other] { $layers } layers
    }
check-stood =
    { $count ->
        [0] No island had an underside to stand a support on
        [one] Stood { $count } support in { $group }
       *[other] Stood { $count } supports in { $group }
    }
check-fact-checking = checking
check-fact-none = no issues
check-fact-issues =
    { $count ->
        [one] { $count } issue
       *[other] { $count } issues
    }
check-checking = Checking
check-reading = Reading every layer of the stack...
check-reads-layers = The check reads the layers, which are cut as soon as there is a model on the plate and a machine to cut it for.
check-measuring = measuring
check-hardest-pull = Hardest pull
check-disc = Pulls like a disc of
check-widest-step = Widest step
check-grows-by = Grows by
check-layer-lower = layer { $layer }
check-peel = Peel
check-nothing-found = Nothing found to fail on
check-nothing-found-why = No islands, no neck pulled past its limit, and no layer pulling the film too hard.
check-issues-help = Islands start in mid-air and stay stuck to the film. Levers are thin necks a pull to one side can snap, cups are closed hollows the lift sucks on, and thin sections may not form at all. Click a row to go to its layer.
check-peel-help = Each lift tears the fresh layer off the film, and a wider layer pulls harder. The hardest pull is the worst layer, shown as the disc that would pull the same; the widest step is where the area grows most from one layer to the next. Tilting or hollowing the model lowers both.
check-worst = { $issue } on layer { $layer }
check-put-islands-back = Put the islands back
check-remove-islands =
    { $count ->
        [one] Remove { $count } island
       *[other] Remove { $count } islands
    }
check-remove-islands-note = Erases them from the written file and checks again. Only the islands go; what stood on them stays, and is listed to be supported.
check-hanging-passes =
    { $count ->
        [one] And { $count } layer more of what stood on them.
       *[other] And { $count } layers more of what stood on them.
    }
check-remove-next =
    { $count ->
        [one] Remove the next { $count } island
       *[other] Remove the next { $count } islands
    }
check-remove-next-note = Takes one layer more off each part left hanging, the one it starts on, and checks again.
check-no-islands-to-take = No islands to take out
check-erased-note = Erased from the file; anything left hanging is listed below.
check-layer = Layer { $layer }
check-more-layers = and { $count } more layers
check-row-hanging = hangs over nothing · { $area } mm2
check-row-islands =
    { $count ->
        [one] island · { $area } mm2
       *[other] { $count } islands · { $area } mm2
    }
check-row-lever = lever · { $lever } mm · { $stress } MPa
check-row-levers = { $count } levers · { $stress } MPa
check-row-cup = suction cup · { $area } mm2
check-row-cups = { $count } suction cups · { $area } mm2
check-row-thin =
    { $count ->
        [one] thin · { $width } mm
       *[other] { $count } thin · { $width } mm
    }
check-row-peel = peel · { $force } N
check-issue-island = Unsupported island
check-issue-hanging = Part left hanging
check-issue-lever = Weak neck
check-issue-peel = Hard peel
check-issue-cup = Suction cup
check-issue-thin = Thin section
check-explain-hanging = An island under it was taken out, so { $area } mm2 here hangs over nothing and the rest of that part with it. Support it.
check-explain-island = An island of { $area } mm2 cures over nothing: it sticks to the film and stays in the vat. Support it, or remove it.
check-explain-lever = The neck under it, { $neck } mm2, takes about { $stress } MPa from a pull { $lever } mm off to one side. Support it nearer its middle.
check-explain-peel = The film pulls on this layer with about { $force } N. Tilt the model, hollow it, or lift slower.
check-explain-cup = A hole of { $area } mm2 holding { $volume } ml is sealed from below, so each lift pulls on it. A drain hole lets the air in.
check-explain-thin = A section { $width } mm across stands over a millimetre tall. It may not form; thicken it if it matters.
check-removed =
    { $count ->
        [one] { $count } island taken out of the file
       *[other] { $count } islands taken out of the file
    }

## panels/plate_summary.rs

summary-layer = Layer { $layer }
summary-this-plate = This plate
summary-layer-range = { $fewest } to { $most }
summary-height = Height
summary-layers = Layers
summary-vertices = Vertices
summary-triangles = Triangles
summary-model-supports = Model/supports
summary-price = Price
summary-z-height = Z height
summary-exposure = Exposure
summary-cured-area = Cured area
summary-not-priced = not priced
summary-volume = Volume
summary-weight = Weight
summary-measuring = measuring
summary-time = Time
summary-block-bottom = bottom block
summary-block-transition = transition
summary-block-body = body

## ui/widgets.rs

duration-hours = { $hours }h{ $minutes }m{ $seconds }s
duration-minutes = { $minutes }m{ $seconds }s
ago-never = never
ago-just-now = just now
ago-minutes = { $count } min ago
ago-hours = { $count } h ago
ago-days =
    { $count ->
        [1] yesterday
       *[other] { $count } days ago
    }

## ui/widgets/field.rs

field-step-more = { $step } { $unit } more
field-step-less = { $step } { $unit } less
field-back-to = Back to { $value } { $unit }
field-printer-address = Printer address

## ui/widgets/layout.rs

dialog-close = Close  Esc

## ui/widgets/list.rs

list-hide = Hide in the view
list-show = Show in the view

## panels/inspector/measure.rs

measure-settings = Settings
measure-measure = Measure
measure-esc-clears = Esc clears.
measure-clear = Clear
measure-points = Points
measure-planes = Planes
measure-snap-to = Snap to
measure-snap-corner = Corner
measure-snap-edge = Edge
measure-snap-free = Free
measure-snap-corner-note = The nearest corner where the surface turns catches a click.
measure-snap-edge-note = The nearest edge where the surface turns catches a click.
measure-snap-free-note = Exactly where the click lands.
measure-free-while-alt = Free while Alt is held.
measure-hold-alt = Hold Alt to measure freely.
measure-show-axes = Show dX, dY, dZ in the view
measure-second-point = Click the second point.
measure-two-points = Click two points on a model.
measure-distance = Distance
measure-second-face = Click the second flat face.
measure-two-faces = Click two flat faces on a model.
measure-gap = Gap
measure-angle = Angle
measure-between-centres = Between centres
measure-not-parallel = Not parallel, so there is no one gap between them.

## panels/inspector/hollow.rs

hollow-shell = Shell
hollow-infill = Infill
hollow-keep-solid = Keep solid
hollow-keep-solid-note = Click the model to keep its wall solid there, alt-click a blocker to take it away.
hollow-drainage = Drainage
hollow-looking = Looking for trapped resin
hollow-drains = The resin can get out of every cavity on the plate.
hollow-trapped =
    { $count ->
        [one] Resin is trapped in { $count } place
       *[other] Resin is trapped in { $count } places
    }
hollow-trapped-note = Each one needs a hole of its own. Turn the x-ray on to see the space it fills.
hollow-drain-trapped = Drain the trapped resin
hollow-inside = Inside
hollow-outside = Outside
hollow-inside-note = The wall grows inward; the outside is untouched.
hollow-outside-note = The wall grows outward, making a mould of the model.
hollow-shell-help = A hollow model takes less resin and pulls less on the film. The wall is how thick its skin stays; about 2 mm suits most parts. A cavity needs a drain hole, or the resin trapped inside can crack the part later.
hollow-wall = Wall
hollow-precision = Precision
hollow-precision-note = Precision is how smooth the cavity comes out, on a { $lattice } mm lattice. It is not the layer height.
hollow-empty = The cavity is left empty.
hollow-hive = Hive
hollow-grid = Grid
hollow-scaffold = Scaffold
hollow-cell = Cell
hollow-density = Density
hollow-infill-note = { $walls } mm walls. Hive and Grid stand on the plate; Scaffold also braces sideways. Every cell is open at the cavity's floor and ceiling, so resin runs from cell to cell towards a hole.
hollow-blocker-size = Blocker size
hollow-clear-blockers =
    { $count ->
        [one] Clear { $count } blocker
       *[other] Clear { $count } blockers
    }
hollow-tooltip = Cuts a cavity inside { $scope }.
hollow-hollow = Hollow
hollow-hollowing = Hollowing
hollow-hollowed = Hollowed
hollow-resin-saved = Resin saved
hollow-coarsened = This model needed more memory than the budget allows, so the cavity was cut on a { $lattice } mm lattice.
hollow-stale = The numbers have moved since. Hollow again to catch up.
hollow-make-solid = Make solid again

## panels/inspector/print_settings.rs

print-bottom-layers = Bottom layers
print-exposure = Exposure
print-bottom-exposure = Bottom exposure
print-light-off = Light off
print-before-lift = Before lift
print-after-lift = After lift
print-after-retract = After retract
print-lift = Lift
print-lift-speed = Lift speed
print-retract-speed = Retract speed
print-this-session = this session
print-from-resin = From the resin profile. Edits change this session's slices only; the resin profile keeps its own numbers.
print-exposure-help = How long the screen lights each layer. Too short and thin parts fail or layers come apart; too long and detail swells and holes close up. The bottom layers are lit longer so the print sticks to the plate, and transition layers step down between the two.
print-waiting-help = Pauses that let the resin settle. Light off is one wait before each layer is lit; rest times wait after the exposure, at the top of the lift and once the plate is back. Longer waits help thick resin and large layers, at the cost of print time.
print-motion-help = After each layer the plate lifts to peel it off the film, then comes back down. A slower lift peels gentler, which large or fragile layers need, and the lift has to be high enough for the film to let go.
print-waiting = Waiting
print-motion = Motion
print-carried = Carried from { $exposure } s at { $from } mm to the { $to } mm layer.
print-transition = Transition
print-transition-layers = Transition layers
print-transition-type = Transition type
print-linear = Linear
print-step = Step between layers
print-resting = Resting

## panels/inspector/relief.rs

relief-tile-size = Tile size
relief-angle = Angle
relief-depth = Depth
relief-precision = Precision
relief-image = Image
relief-image-note = A model's own texture becomes geometry: white moves the surface out, black leaves it where it is. Or choose a picture to tile over any model.
relief-picture-size = { $width } × { $height } px
relief-choose-another = Choose another
relief-choose-picture = Choose a picture
relief-own-textures = Use the models' own textures
relief-tiling-note = The picture is laid from the three sides the model faces, and each face reads it from the side it faces most.
relief-lattice-note = The relief is cut on a { $lattice } mm lattice, and the whole model is meshed again at it. A negative depth sinks the texture in.
relief-no-texture = No texture on the plate. It is read from the file a model is opened from, and is gone once it has been pressed in.
relief-textures =
    { $count ->
        [1] Texture
       *[other] { $count } textures
    }
relief-pressing = Pressing...
relief-the-texture = the texture
relief-tooltip = Presses { $picture } into { $scope }. The model is replaced, so its supports and its cavity go with it.
relief-press = Press it in
relief-pressing-status = Pressing the relief

## panels/title_bar.rs

menu-file = File
menu-edit = Edit
menu-view = View
chip-machine = Machine, { $name }
chip-resin = Resin, { $name }
chip-machine-tooltip = The machine this plate is printed on
chip-resin-untuned = The resin, not measured on this printer
chip-resin-tooltip = The resin
chip-layers = { $layer } µm, { $exposure } s
find-button = Find
find-tooltip = Find a tool, an action or a model
bar-source = Source
bar-source-tooltip = Encrust is free software under the AGPL-3.0; this is the code it runs
bar-close = Close
bar-restore = Restore
bar-maximise = Maximise
bar-minimise = Minimise
bar-restart-to-update = Restart to update
bar-restart-tooltip = Close the window and start the new build
bar-update-tooltip = A new release is out: see what changed
bar-alpha = Alpha
bar-alpha-tooltip = Encrust is in alpha: say what went wrong
menu-new-plate = New plate
menu-new-project = New project
menu-open-project = Open project...
menu-open-recent = Open recent
menu-save-project = Save project
menu-save-project-as = Save project as...
menu-open-model = Open model...
menu-open-test-print = Open test print
menu-import-machine = Import a machine...
menu-import-resin = Import a resin...
menu-machine-and-resin = Machine and resin...
menu-settings = Settings...
menu-quit = Quit
menu-nothing-recent = Nothing opened yet
menu-arrange-plate = Arrange plate
menu-auto-orient-plate = Auto-orient plate
menu-move-to-plate = Move to plate
menu-frame-view = Frame view
menu-plate-grid = Plate grid
menu-xray = See through models
menu-orthographic = Orthographic view

## panels/support_fields.rs

support-contact = Contact
support-bite = Bite
support-length = Length
support-diameter = Diameter
support-clearance = Clearance
support-trunk-cap = Trunk cap
support-thickness = Thickness
support-flare = Flare
support-upper-depth = Upper depth
support-lower-depth = Lower depth
support-max-length = Max length
support-branch-angle = Branch angle
support-merge-within = Merge within
support-margin = Margin
support-wall-slope = Wall slope
support-span = Span
support-first-at = First at
support-every = Every
support-lift = Lift
support-overhang = Overhang
support-contact-spacing = Contact spacing
support-density = Density
support-group-top = Top
support-group-pillar = Pillar
support-group-bottom = Bottom
support-group-small-pillar = Small pillar
support-group-branching = Branching
support-group-raft = Raft
support-group-bracing = Cross bracing
support-group-automatic = Automatic
support-shape-plane = Plane
support-shape-sphere = Sphere
support-foot-round = Round
support-foot-tapered = Tapered
support-foot-prism = Prism
support-foot-cube = Cube
support-foot-skate = Skate
support-land-on-model = Stand on the model
support-land-on-model-off = A support with the model under it leans aside for the plate, or is dropped. Nothing is left cured onto the part.
support-thin-down = Thin these down
support-thin-down-off = A support that lands on the model keeps the regular pillar's width.
support-small-pillar-note = Used where a support runs from one part of the model to another. Both ends sink into a surface; a longer run keeps the regular width.
support-no-branching = Every tip stands on a column of its own.
support-tip-help = The only part that touches the model. A wider contact holds better but leaves a bigger mark; the bite sinks it in so it does not tear off mid-print. Length is the cone from the tip to the column, and a sphere grips a heavy overhang harder than a plane.
support-body-help = Diameter is how thick the column is: thicker is stiffer and takes more resin. Clearance is the air the column keeps from the model, so it never cures onto the part. Trunk cap is the widest a trunk may grow at its base.
support-foot-help = The foot holds the support to the plate. Wider and thicker grips harder and is harder to take off; Tapered and Skate leave an edge a scraper gets under. Flare is how wide the column spreads before it meets the foot.
support-branching-help = Nearby tips merge into shared trunks, like a tree: fewer columns, less resin and fewer marks on the plate. A wider branch angle merges more but leans the branches further; tips farther apart than Merge within keep columns of their own.
support-raft-help = A slab the supports stand on instead of the plate: it spreads the peel over one footprint and comes off in one piece. Grid sucks less on the plate, Connected joins a pad under each foot. Margin is how far it reaches past the model, and a sloped wall grips harder yet still pries off.
support-bracing-help = Struts that tie tall trunks together so they do not sway when a layer peels. Span is the widest gap they bridge; First at and Every set the height of the first one and the step between them.
support-held-up-help = Overhang is how far a surface may lean from vertical before it gets supports: 90° is a ceiling, and a lower number supports more. Contact spacing is the distance between tips over an overhang, and density multiplies how many go down: 1 is what Encrust reckons is enough.
support-lift-help = How high the model stands off the plate. Supports need room under it, and the first layers stick to the plate hardest, so a part printed straight on it is hard to free.
support-raft = Raft under the feet
support-raft-skates = Skates
support-raft-grid = Grid
support-raft-connected = Connected
support-raft-hull = Hull
support-raft-rectangle = Rectangle
support-bracing = Brace tall trunks

## panels/settings/supports.rs

support-table-tip = Tip
support-table-spacing = Spacing
support-cannot-add = cannot add a support profile
support-pick-profile = Pick a support profile.
support-cannot-save = cannot save the support profile

## panels/inspector/shape.rs

shape-part = Part
shape-regular = Regular
shape-raft-own = The raft is this model's own, brought by its project.
shape-raft-plate = The raft is the plate's: every model stands on it.
shape-use-plate-raft = Use the plate's raft
shape-use-profile-raft = Use the raft of { $name }
shape-edited = { $name } (edited)
shape-edit-profiles = Edit support profiles
shape-save = Save
shape-save-tooltip = Save over the profile these numbers were taken from
shape-save-as-new = Save as new
shape-save-as-new-tooltip = Keep these numbers as a profile of their own
shape-saved = Saved the support profile
shape-profile-help = A profile is a saved set of support sizes. Picking another changes what grows next; supports already standing keep their shape. Changed numbers mark it as edited until Save writes over it or Save as new keeps a copy.

## panels/support_diagram.rs

diagram-tip = Tip
diagram-body = Body
diagram-base = Base
diagram-raft = Raft
diagram-brace = Brace
diagram-no-branching = no branching
diagram-no-raft = no raft
diagram-every = every { $value }
diagram-first = first { $value }
diagram-no-bracing = no bracing
diagram-max = max { $value }
diagram-model = model
diagram-plate = plate

## panels/settings/presets.rs

presets-blocker = Blocker
presets-depth = Depth
presets-taper = Taper
presets-empty =
    { $kind ->
        [hollow] No hollow profile yet. Keep one from the tool's Profile row, or import one.
       *[drain] No drain profile yet. Keep one from the tool's Profile row, or import one.
    }
presets-none = None
presets-cannot-rename = cannot rename the profile

## panels/inspector/presets.rs

presets-unsaved = Unsaved
presets-none-saved = None saved yet
presets-delete-named = Delete { $name }
presets-deleted =
    { $kind ->
        [hollow] Deleted the hollow profile
       *[drain] Deleted the drain profile
    }
presets-name-hint = Name these settings

## panels/settings/machines/resins.rs

resins-type = Type
resins-layer = Layer
resins-bottom = Bottom
resins-every = Every resin
resins-by-hand = The exposure and layers off the bottle or the resin maker's site
resins-add = Add a resin
resins-from-file = A file sliced for this machine (.goo, .ctb, ...) or a resin profile (.cfgx, .ini), or drop one here
resins-import = Import a resin
resins-all-types = All types
resins-from-pool = From the pool
resins-delete = Delete this resin
resins-all-here = Every resin you have is already on this machine.
resins-none-here = No resin on this machine yet. Add one with Add a resin above.
resins-row-in-hand = The plate prints with this resin. Double-click to edit it.
resins-row = Double-click to edit it.
resins-take-off = Take off this machine
resins-use = Use this resin

## files.rs

filter-model = Model
filter-mesh = Mesh
filter-sliced = Sliced file
filter-model-or-project = Model or project
filter-project = Encrust project
filter-printer-profile = Printer profile
filter-resin = Resin profile or sliced file
filter-picture = Picture
filter-profile = Profile

## panels/settings/resin.rs

resin-density = Density
resin-layer-height = Layer height
resin-retract = Retract
resin-bottom-lift = Bottom lift
resin-bottom-lift-speed = Bottom lift speed
resin-bottom-retract-speed = Bottom retract speed
resin-resin = Resin
resin-exposure-on = Exposure on the { $printer }
resin-waits = Waits
resin-compensation = Compensation
resin-cannot-save = cannot save the resin
resin-cannot-rename = cannot rename the resin
resin-colour = Colour
resin-per = per
resin-per-kg = kg
resin-per-litre = l
resin-light-off-delay = Light-off delay
resin-rest-times = Rest times
resin-waits-note = Counted into the print time, and written into the file.
resin-carried = Carried from { $exposure } s at { $from } mm to { $to } mm.
resin-light-power = Light power
resin-bottom-power = Bottom power
resin-transition-note = Linear: each layer { $step } s shorter than the one under it.

## panels/settings/compensation.rs

comp-every-second-layer = Every second layer
comp-unaccounted = Unaccounted per layer
comp-shrinkage = Shrinkage
comp-from-measured = Work it out from a measured part
comp-tolerance = Tolerance
comp-bottom-tolerance = Bottom tolerance
comp-print-time = Print time
comp-from-print = Work it out from a finished print
comp-shrinkage-title = Shrinkage from a measured part
comp-layer-time-title = Unaccounted time from a print
comp-shrinkage-note = A test part printed at 100 %, cured and measured cold.
comp-drawn = Drawn
comp-measured = Measured
comp-scale = Scale
comp-use-these = Use these
comp-limit = The field takes { $least } to { $most } { $unit }, so it will be held at { $kept }.
comp-same-print = Both times from the same print.
comp-predicted = Predicted
comp-actual = Actual
comp-per-layer = Per layer
comp-use-this = Use this
comp-hours = hours
comp-minutes = minutes
comp-seconds = seconds

## panels/inspector/layers.rs

layers-cusp = Cusp
layers-thinnest = Thinnest layer
layers-from = From
layers-to = To
layers-thickest = Thickest layer
layers-bands =
    { $count ->
        [one] { $count } band
       *[other] { $count } bands
    }
layers-by-height = By height
layers-levels = { $count } levels
layers-edge-smoothing = Edge smoothing
layers-edge-note = Grey levels on the mask edge, to hide the pixel stairs.
layers-look = Look
layers-number = Number
layers-anti-aliased = Anti-aliased masks
look-off = Off
look-sharp = Sharp
look-balanced = Balanced
look-soft = Soft
look-off-line = Raw masks
look-sharp-line = Crisp text
look-balanced-line = The default
look-soft-line = Organic curves
layers-by-hand = The numbers were set by hand. Pick a look to replace them.
layers-open-printer = Open printer settings
layers-planes = Planes a layer
layers-planes-note = a feature thinner than a layer survives, at that many times the cut
layers-height-help = Thinner layers show fewer steps on curved surfaces but take longer: the time grows with the layer count. Adaptive height cuts thin layers where the surface curves and thick ones where it runs straight.
layers-round-grey = Round edge grey
layers-grey-levels = Grey levels
layers-edge-blur = Edge blur
layers-blur-note = a wide blur dims the wall inside the edge as well; one or two pixels is usual
layers-carried = Exposure carried with the layer: { $was } s at { $from } mm is { $now } s now.
layers-thickest-reached = The thickest layer reached is { $thickest } mm, not { $asked } mm, so a normal layer takes { $exposure } s rather than { $resin } s.
layers-adaptive = Adaptive height
layers-adaptive-refused = This printer steps the plate by the header's height, so no layer can have one of its own.
layers-no-bands = Every layer takes the resin's exposure. A band replaces it over a height.
layers-bands-note = The bottom block keeps the resin's own exposure whatever a band says.
layers-bands-refused = This printer reads the header alone, so exposure cannot vary by height.
layers-add-band = Add a band
layers-band = Band { $index }
layers-remove-band = Remove this band

## panels/settings/printer.rs

printer-panel-width = Panel width
printer-panel-height = Panel height
printer-lit-width = Lit width
printer-lit-height = Lit height
printer-volume-x = Volume X
printer-volume-y = Volume Y
printer-volume-z = Volume Z
printer-per-layer = Per-layer settings
printer-variable-height = Variable layer height
printer-manufacturer = Manufacturer
printer-display = Display
printer-build-volume = Build volume
printer-output = Output
printer-firmware = Firmware
printer-browser-sends-nothing = A browser cannot reach a printer on your network. What is sliced downloads, and goes over on a stick or through the printer's own app.
printer-own-copy = Your copy of a machine this build also ships, and the two differ. Yours wins, including where a later release corrected the shipped numbers.
printer-use-shipped = Use the shipped profile
printer-cannot-put-back = cannot put the shipped profile back
printer-cannot-save = cannot save the printer
printer-grey-floor = Grey floor
printer-grey-floor-off = every grey is written; a pixel too dim to cure leaves liquid resin
printer-grey-floor-on = a dimmer pixel is written black, so an edge loses a fraction of a pixel
printer-pitch = { $x } x { $y } um a pixel
printer-mirror-x = Mirror across X
printer-mirror-y = Mirror across Y
printer-mirror-note = A mask printed the wrong way round is a mirrored part.
printer-per-layer-on = exposure and motion are read from each layer; .goo calls this advance mode
printer-per-layer-off = every layer prints at the header's numbers, and the machine ramps the bottom block itself
printer-variable-height-note = Off means the plate steps by the header's height, so an adaptive stack is refused.
printer-output-note = What this machine is sliced into, dialog and printer alike.

## network.rs

reach-answered = { $name } answered the last scan
reach-silent = { $name } did not answer the last scan
reach-unasked = { $name } has not been asked yet
network-advice-app = If it is switched on, macOS may be keeping Encrust off the local network: allow it under System Settings, Privacy & Security, Local Network.
network-advice-terminal = If it is switched on, macOS may be keeping this window off the local network. Started from a terminal, it has that terminal's permission: start it from Terminal.app, allow your terminal or editor under System Settings, Privacy & Security, Local Network, or run it as an app with `cargo xtask app`.
network-not-reached = not reached yet
network-printing = { $printer } is printing { $file }
network-busy = { $name } is busy: { $state }
network-prusa-sl1 = A Prusa machine prints .sl1; set that as this printer's output.
network-sending = Sending to { $name }
network-starting = Starting { $file } on { $name }
network-sent = { $file } is on { $printer }
network-cancelled = Transfer cancelled
network-usb-only = USB only

## panels/settings/printer/connection.rs

connection-usb-note = What is sliced is saved to a file and carried over on a stick.
connection-found = Found on the network
connection-scanning = Scanning...
connection-scan-again = Scan again
connection-add = Add
connection-address-hint = Add by address, 192.168.1.42
connection-listed = Printers this machine sends to
connection-none-listed = None yet: tick them below, or add one by its address.
connection-speaks = Speaks { $theirs }, not { $ours }
connection-not-found = Not found on the network
connection-not-looked = Not looked for yet
connection-take-off = Take off this machine's list
connection-looking = Looking for printers on the network
connection-all-listed = Every printer found is listed.
connection-other-model = Another model than this profile's
connection-add-all = Add all { $count } of this model
connection-set-up-here = Set up in this window
connection-forget-login = Forget the login for { $host }
connection-set-up-another = Set up another
connection-login-note = The login is kept in this window's settings, not in the profile.
connection-password = password
connection-api-key = or an API key

## panels/printer_card.rs

card-open-printer = Open the printer
card-search-failed = The search could not go out on any network
card-nothing-answered = Nothing answered at { $address }
card-destination-several = The printer this profile sends to; press for the others it lists
card-destination = The printer this profile sends to
card-not-answering = Not answering
card-connecting = Connecting
card-not-asked = Not asked yet
card-paused = Paused
card-ready = Ready
card-stopped-error = Stopped: { $error }
card-finished = Finished
card-stopped = Stopped
card-last-print = Last print
card-layer-of = Layer { $layer } of { $count }
card-light-source = Light source
card-hood = Hood
card-celsius = { $name } { $value } °C
card-almost-done = Almost done
card-minutes-left = { $minutes } min left
card-hours-left = { $hours } h left
card-time-left = { $hours } h { $minutes } min left

## panels/slice.rs

slice-to = Slice to .{ $extension }
slice-tooltip = Rasterise the plate into a printable file
slice-more-tooltip = Slice every plate, or slice and send
slice-all = Slice all { $count } plates
slice-send = Slice and send to { $name }
slice-broken =
    { $count ->
        [one] { $count } model on this plate is broken; what is sliced from it may not print
       *[other] { $count } models on this plate are broken; what is sliced from them may not print
    }
slice-cannot-send = cannot slice the stack to send
slice-slicing = Slicing
slice-all-started = Slicing { $count } plates beside { $path }
slice-cannot-slice-into = cannot slice into { $path }
slice-slicing-into = Slicing into { $path }

## slicing.rs

slicing-cannot-read-profiles = cannot read your profiles in { $dir }
slicing-cannot-read-profiles-browser = cannot read your profiles in this browser
slicing-select-resin = Select a resin
slicing-pick-machine = Pick a machine to know the panel to slice for.
slicing-add-resin = Add a resin to this printer: an exposure is measured on your machine.
slicing-nothing-visible = Nothing visible on the plate to slice.
slicing-steps-by-header = This printer steps the plate by the header's layer height.
slicing-no-printer = no printer profile is loaded
slicing-no-resin = no resin is chosen, and an exposure is measured rather than shipped
slicing-nothing-visible-error = nothing visible on the plate to slice
slicing-no-format = { $path } names no sliced-file format
slicing-clipped = , { $count } layers cut off at the edge of the panel
slicing-wrote = Wrote { $path } : { $layers } layers, { $volume } ml, { $weight } g{ $cost }{ $clipped }
slicing-cancelled = Slicing cancelled

## panels/inspector/export.rs

export-on = on { $printer }
export-on-machine = The file is on the machine. Nobody is standing at it yet.
export-start-print = Start the print
export-slice-all-to = Slice all { $count } plates to .{ $extension }
export-the-file = The file
export-file-name = File name
export-print-settings = Layers, exposure and edges
export-all-plates = All { $count } plates
export-this-plate = This plate
export-send-to-printer = Send to a printer
export-look-again = Look again
export-usb-only = This printer's profile takes files on a stick; choose its network in its settings to send from here.
export-none-listed = No printer of this model is listed to send to yet.
export-choose-them = Choose them in its network settings
export-speaks-other = { $name } speaks { $theirs }, but this profile sends over { $ours }: set its network in the printer's settings.

## hollow.rs

hollow-nothing-visible = Nothing visible on the plate to hollow.
hollow-nothing-visible-error = nothing visible on the plate to hollow
hollow-done =
    { $count ->
        [0] Nothing on the plate to hollow
        [one] Hollowed { $count } model, saving { $saved } ml
       *[other] Hollowed { $count } models, saving { $saved } ml
    }
hollow-cancelled = Hollowing cancelled

## orient.rs

orient-nothing-visible = Nothing visible on the plate to orient.
orient-nothing-visible-error = nothing visible on the plate to orient
orient-pose = Pose { $index } of { $count }
orient-stepped =
    { $count ->
        [0] No other good pose: change the preset, the supports or the lift to search again
        [one] Stood { $count } model its next good way
       *[other] Stood { $count } models their next good way
    }
orient-turned =
    { $count ->
        [0] Everything already stands the way it prints best
        [one] Turned { $count } model
       *[other] Turned { $count } models
    }
orient-seated =
    { $count ->
        [one] { $turned }; { $count } stands on the plate
       *[other] { $turned }; { $count } stand on the plate
    }
orient-others =
    { $count ->
        [one] { $turned }; Auto-orient again for { $count } other good pose
       *[other] { $turned }; Auto-orient again for { $count } other good poses
    }
orient-supports-ml = { $value } ml of supports
orient-pull = { $value } N of pull
orient-tall = { $value } mm tall

## relief.rs

relief-nothing-visible = Nothing visible on the plate to press a texture into.
relief-no-textures = None of these models carries a texture. Choose a picture to tile over them, or open an OBJ with its image beside it, or a 3MF with one inside it.
relief-nothing-to-press = nothing on the plate carries a texture to press in
relief-done =
    { $count ->
        [one] Pressed { $depth } mm of relief into { $count } model
       *[other] Pressed { $depth } mm of relief into { $count } models
    }
relief-done-coarsened =
    { $count ->
        [one] Pressed { $depth } mm of relief into { $count } model; { $coarsened } came out on a coarser lattice than precision asked for
       *[other] Pressed { $depth } mm of relief into { $count } models; { $coarsened } came out on a coarser lattice than precision asked for
    }
relief-pressing-picture = Pressing { $name }

## panels/inspector/position.rs

position-global = Global
position-local = Local
position-move = Move
position-move-reset = Back to the middle of the plate
position-move-along = Move along { $axis }
position-rotation = Rotation
position-rotation-reset = Back to the way the model was imported
position-rotation-about = Rotation about { $axis }
position-scale = Scale
position-scale-reset = Back to the size the model was imported at
position-size = Size
position-ratio = Ratio
position-keep-proportions = Keep the proportions
position-axis-size = { $axis } size
position-axis-ratio = { $axis } ratio
position-place = Place
position-on-plate = On the plate
position-centre = Centre
position-scale-to-fit = Scale to fit
position-lift = Lift off the plate
position-click-face = Click a face
position-lay-on-face = Lay on face
position-auto-orient = Auto-orient
orient-preset-supports = Fewest supports
orient-preset-peel = Least peel force
orient-preset-shortest = Shortest print
orient-preset-auto = Auto
orient-preset-supports-hint = The least resin in supports, whichever way up.
orient-preset-peel-hint = The lightest pull on the film, no cups; under 10 N, the fewest supports.
orient-preset-shortest-hint = The fewest layers that still print.
orient-preset-auto-hint = Supports, pull and print time balanced: none gives up much for the others.
position-mirror = Mirror
position-across = Across { $axis }
position-keep-original = Keep the original
position-keep-original-note = Mirror a copy beside it instead.
position-place-help = On the plate stands the model on it, Centre moves it to the middle, and Scale to fit makes it as large as the build volume allows. Auto-orient turns it the way it prints best; Lay on face turns the face you click down onto the plate.

## panels/inspector/paint.rs

paint-brush = Brush
paint-surface-angle = Surface angle
paint-inside = Inside
paint-hint-point = Click to place, alt-click to remove.
paint-hint-paint = Drag to paint, ctrl-click a surface, alt to erase.
paint-hint-block = Drag to block, ctrl-click a surface, alt to erase.
paint-hint-erase = Drag to take paint and blocks off, ctrl-click a surface.
paint-brush-help = Place puts one support where you click. Fill paints an area that gets supports when you press Fill painted areas; Block paints where no support may stand. Surface angle is how far a ctrl-click spreads: over every face turned less than that from the one clicked.
paint-fill-help = How far apart the supports of a filled area stand: Rim along its edge, Inside across the rest. Smaller is denser and holds better, but leaves more marks.
paint-groups-help = A group is a set of supports built to one profile, so one model can carry thick supports under heavy parts and thin ones under details. New supports join the group in hand.
paint-rim = Rim
paint-groups = Groups
paint-fill = Fill painted areas
paint-filled =
    { $count ->
        [0] The painted areas had nowhere to put a support
        [one] Filled the painted areas with { $count } support
       *[other] Filled the painted areas with { $count } supports
    }
paint-clear-blocked = Clear blocked areas
paint-clear-both = Clear painted and blocked areas
paint-clear-painted = Clear painted areas
paint-drop-group = Drop this group
paint-build-to = Build this group to
paint-group-holds =
    { $count ->
        [one] { $name } holds { $count } support, which goes with it.
       *[other] { $name } holds { $count } supports, which go with it.
    }
paint-drop-the-group = Drop the group
paint-dropped =
    { $count ->
        [one] Dropped { $name } and { $count } support
       *[other] Dropped { $name } and { $count } supports
    }
paint-keep-it = Keep it
paint-new-group = New group

## panels/inspector/drain.rs

drain-hole-size = Hole size
drain-trapped-resin = Trapped resin
drain-opening = Opening
drain-opening-note = Click the model to put one down, alt-click a hole to take it away. Each one is cut the moment it is placed, hollow or not.
drain-fact = { $count } trapped
drain-dig = Dig the channel ({ $count } points)
drain-dig-tooltip = A tube of the hole size runs under every point, open at its two ends alone.
drain-dug =
    { $count ->
        [one] { $count } channel dug
       *[other] { $count } channels dug
    }
drain-check-again = Check again
drain-check = Check drainage
drain-check-tooltip = Cuts the plate and follows the air through it, looking for resin with no way out.
drain-checking = Checking drainage
drain-hole = Hole
drain-channel = Channel
drain-channel-note = Each click adds a point; the channel is a tube of the hole size running under all of them, open at its two ends alone.
drain-sizes-note = The depth is measured from the surface inwards, so a hole has to be deeper than the wall. A taper under 1 makes it a cone.
drain-none-cut = No hole and no channel is cut yet.
drain-clear-cuts =
    { $count ->
        [one] Clear { $count } cut
       *[other] Clear { $count } cuts
    }
drain-rehollowing = Hollowing again around the channels
drain-nothing-trapped = Nothing is trapped: the resin can get out of every model on the plate.
drain-not-checked = Not checked yet.
drain-held = { $volume } ml cannot get out
drain-held-note = It cures inside and adds weight the peel has to pull. Each pocket is painted red under the x-ray, and each needs a hole of its own.
drain-holes-placed =
    { $count ->
        [one] { $count } hole placed
       *[other] { $count } holes placed
    }
drain-drill-each = Drill a hole into each
drain-lowest-point = Each hole goes in at the lowest point of its pocket.
drain-pocket = pocket { $index }
drain-drill = Drill
drain-drill-tooltip = A hole at this pocket's lowest point

## updates.rs

update-error-fetch = cannot fetch { $url }
update-error-feed = the update feed is not readable
update-error-version = `{ $version }` is not a release version
update-error-foreign = the feed points outside this project's releases: { $url }
update-error-signature = the download is not signed with the Encrust release key
update-error-signed = the signature is for `{ $found }`, not `{ $expected }`
update-error-archive = the downloaded archive cannot be unpacked
update-error-missing = the downloaded archive holds no `{ $name }`
update-error-replace = cannot replace { $path }
update-error-store = the Microsoft Store did not answer
update-no-build = This release has no build for this platform.
update-cannot-check = cannot check for updates
update-installed = Encrust { $version } is installed
update-cannot-install = cannot install Encrust { $version }
update-cannot-open-store = cannot open the Microsoft Store

## updates/install.rs

update-dev-build = A development build is updated by building it.
update-no-exe = The running program cannot be located.
update-by-store = This copy is updated by the Microsoft Store.
update-by-package = This copy is updated by downloading the new package.

## panels/settings/updates.rs

updates-checking = Checking
updates-by-store = The Microsoft Store keeps this copy up to date.
updates-store-link = Encrust in the Microsoft Store
updates-daily = Look for a new release once a day
updates-privacy = One request a day to GitHub for the newest release. It carries this build's version and nothing else, and GitHub sees your address. Nothing is downloaded or installed until you ask.
updates-check-now = Check now
updates-this-is = This is { $version }, last looked { $when }
updates-looking = Looking for a new release
updates-newest = This is the newest release.
updates-installing = Downloading and checking the signature
updates-installed = Installed. It runs from the next start.
updates-restart-now = Restart now
updates-store-has-it = The Microsoft Store has this release for this copy.
updates-in-store = Update in the Microsoft Store
updates-install = Install and restart
updates-skip = Skip this version
updates-release-page = Release page
updates-more-notes = And more on the release page.

## panels/start.rs

start-title = Start a plate
start-subtitle = Open a model, or pick up where you left off.
start-zone = Open a model or a project
start-drop-here = Drop a model here
start-drop-formats = STL, OBJ, 3MF, or an .encrust project
start-open-model = Open a model
start-open-project = Open a project
start-recent = Recent
start-test-prints = Test prints
start-files =
    { $count ->
        [one] { $count } file
       *[other] { $count } files
    }
start-models =
    { $count ->
        [one] { $count } model
       *[other] { $count } models
    }
start-nothing-recent = Nothing opened yet. A model or a project you open is shown here.
start-test-print = Test print
start-gone = { $path } is no longer there
start-project = Project

## panels/stage_notice.rs

notice-cutting-preview = Cutting the layers to preview
notice-start-print = Start print
notice-broken-one = { $name } is broken
notice-broken-several = { $count } models on this plate are broken
notice-repair = Repair
notice-repair-all = Repair all
notice-keep-as-is = Keep as it is
notice-no-printer-resin = No printer or resin yet
notice-no-printer = No printer yet
notice-no-resin = No resin on this printer yet
notice-set-up = Set up
notice-put-away = Put away

## project.rs

project-save-and-close = Save and close
project-save-and-new = Save and start a new project
project-close-without-saving = Close without saving
project-new-without-saving = Start a new project without saving
project-cannot-open = cannot open the project { $path }
project-opened-note = Opened { $path }; { $note }
project-cannot-draw = cannot draw the project's picture
project-cannot-write = cannot write the project { $path }
project-unsaved = This plate has changes that are not in a project file.
project-keep-working = Keep working

## project/state.rs

project-missing-printer = the printer { $printer } it names is not among yours, so yours stays
project-missing-resin = the resin { $resin } it names is not among yours, so yours stays
project-missing-both = the printer { $printer } and the resin { $resin } it names are not among yours, so yours stay

## panels/section.rs

view-model = Model
view-side-by-side = Model and layer side by side
view-mask = Layer mask
section-pause = Pause
section-no-layers = No layers
section-no-layers-yet = No layers yet
section-height-of = { $height } of { $top } mm
section-type-layer = Type a layer to go to
section-layer-field = Layer { $layer }, type a layer to go to
section-slider = Layer, of { $count }
section-nothing-to-cut = Nothing on the plate to cut through yet
section-layers-range = Layers { $first } to { $layer }

## panels/settings/confirm.rs

confirm-cannot-undo = This cannot be undone.
confirm-remove-machine = Remove { $name } and everything you changed on it?
confirm-remove = Remove
confirm-machine-stays = The machine stays in the library, so it can be added again with the numbers this build ships.
confirm-take-resin-off = Take this resin off
confirm-delete-named = Delete { $name }?
confirm-untouched-resin = Nothing was ever typed into it, so it is not kept in the pool.
confirm-take-off-named = Take { $name } off this printer?
confirm-take-off = Take off
confirm-stays-in-pool = It stays in the pool, for this or any other printer to take back.
confirm-stays-measured = It stays in the pool, measured on { $printers }.
confirm-delete-resin = Delete this resin
confirm-delete-resin-named = Delete { $name } and the exposures measured for it?
confirm-comes-off = It comes off { $printers } as well.
confirm-delete-profile = Delete this profile

## panels/report.rs

report-nothing-sent = Encrust is in alpha, and a report is how a bug gets fixed. Nothing is sent from here: what you see below stays on this machine until you hand it over yourself.
report-no-model = Your model is never in the report. Attach it to the issue yourself only if it is yours to share — a small shape that shows the same bug is better for everyone.
report-what-happened = What happened, and what you expected instead
report-steps = How to reproduce it
report-title = Report a problem
report-include-profiles = The printer and resin profiles
report-include-settings = What the tool panels are set to
report-include-plate = How much stands on the plate
report-the-report = The report
report-open-issue = Open an issue
report-copied-for-issue = The report is on your clipboard: paste it into the issue
report-copy = Copy the report
report-copied = The report is on your clipboard
report-save = Save it as a file
report-cannot-write = cannot write the report to { $path }
report-cannot-hand-over = cannot hand over the report

## sliced.rs

sliced-no-panel = the file states no panel size and no printer of its panel is known; choose the printer it was cut for first
sliced-no-layers = { $name } has no layers
sliced-layer-unreachable = layer { $layer } of { $name } stands nowhere a printer reaches
sliced-cannot-decode = cannot decode layer { $layer } of { $name }
sliced-no-pixel = { $name } lights no pixel

## scene.rs

defect-open = The surface is not closed: { $count } edges have nothing on the other side
defect-degenerate = { $count } faces with no area
defect-duplicate = { $count } faces drawn twice over
note-folded = { $count } edges where faces lie folded onto each other, sliced as they stand
repaired-faces-removed = Repaired: { $count } collapsed or doubled faces removed
repaired-flipped = Repaired: { $count } faces turned the right way round
repaired-inverted = Repaired: { $count } shells turned the right way out
scene-plate-name = Plate { $index }
scene-copy-numbered = { $name } copy { $count }
# The scope-* messages are what a tool works on, set into the tooltips of the Hollow,
# Supports and Relief buttons as their { $scope }.
scope-selected-one = the selected model
scope-selected = the { $count } selected models
scope-plate-one = the one model on the plate
scope-plate = all { $count } models on the plate

## panels/scene_panel.rs

models-broken = broken
models-title = Models
models-selected = { $count } selected
models-arrange = Arrange the plate
models-duplicate = Duplicate { $count } models
models-array = Array
models-columns = Columns
models-rows = Rows
models-lay-out = Lay out
models-show = Show { $name }
models-import-found = What the import found

## samples.rs

sample-detail = Detail test
sample-detail-about = Spikes, ledges at three angles and the mark's overhangs, all without supports.
sample-exposure = Exposure check
sample-exposure-about = Pins and fins break off underexposed, slots and holes close up overexposed.
sample-surface = Surface finish
sample-surface-about = A dome and a column show the pixel steps and anti-aliasing, a ramp the layers.
sample-fit = Fit gauge
sample-fit-about = Pins 0.3 mm under to 0.1 mm over a 6 mm hole, shortest smallest: which go in.

## job.rs

job-rasterising = Rasterising
job-thread-stopped = the slicing thread stopped without finishing
job-cancelling = Cancelling
job-progress = { $stage } layer { $done } of { $total }
job-working = { $stage }...

## job/pipeline.rs

job-cannot-start-threads = cannot start the slicing threads
job-cannot-slice = cannot slice the model
job-cannot-measure = cannot measure the stack

## panels/inspector.rs

inspector-parallel = parallel
inspector-about-select = Pick a model, then move, turn, size, lay down and mirror it.
inspector-about-measure = Click two points or two flat faces on a model.
inspector-about-hollow = Cut a cavity to save resin and lighten the peel.
inspector-about-drain = Let resin out of a hollow model.
inspector-about-cut = Split a model along a plane.
inspector-about-relief = Press a model's texture into its surface. White is the high point.
inspector-about-supports = Hold up what hangs over nothing.
inspector-about-paint = Tell the supports where to go and where not to.
inspector-about-shape = The shape of one support, part by part.
inspector-about-check = What will fail on the machine, with the layer it fails on.
inspector-about-export = From the plate to a file, and from the file to a printer.
inspector-about-print-settings = How this plate is cut and exposed.

## web/files.rs

web-cannot-open-dialog = cannot open the file dialog: { $error }
web-cannot-hand-over = cannot hand over { $name }: { $error }
web-cannot-read = cannot read { $name }: { $error }
web-nothing-to-open = nothing to open among the files picked; pick a .{ $extensions }

## preview/picture.rs

preview-no-stack = no sliced stack to preview
preview-no-plate = no plate to preview
preview-cannot-cut-under = cannot cut the layer under the one being previewed
preview-not-in-stack = layer { $index } is not in the stack
preview-cannot-rasterise = cannot rasterise the layer at z = { $z } mm

## panels/printer_card/controls.rs

controls-stop-question = Stop the print? It cannot be resumed.
controls-stop = Stop the print
controls-keep-printing = Keep printing
controls-film = Release film { $lifts } of { $max } lifts
controls-copy = Copy
controls-camera-off = Turn the camera off
controls-rtsp = The stream is RTSP: open the address in a player that takes it.
controls-camera = Camera stream

## import.rs

import-opening = Opening { $path }
import-and-more = { $first }, and { $count } more
import-no-extension = { $name } has no file extension
import-cannot-load = cannot load { $name }
import-no-vertices = { $name } has no vertices

## repair.rs

repair-gone = { $name } is no longer on the plate
repair-kept = { $name } could not be repaired without leaving more wrong with it, so it is as it came
repair-nothing = { $name } had nothing left to repair
repair-no-patch = { $name } has { $count } boundaries with no patch that would close them
repair-done = Repaired { $name }: { $what }
repair-done-open = Repaired { $name }: { $what }; { $count } boundaries could not be closed
repair-closed-holes =
    { $count ->
        [one] closed { $count } hole
       *[other] closed { $count } holes
    }
repair-dropped-faces =
    { $count ->
        [one] dropped { $count } face drawn twice
       *[other] dropped { $count } faces drawn twice
    }

## panels/view_cube.rs

cube-top = Top
cube-bottom = Bottom
cube-front = Front
cube-back = Back
cube-right = Right
cube-left = Left
cube-home = Home view

## panels/mask_pane.rs

mask-pick-machine = Pick a machine to know the panel to draw the layers on.
mask-no-stack = No stack to show yet.
mask-zoomed = x{ $zoom }   double-click to fit
mask-how = scroll to zoom, drag to move
mask-caption = Layer mask   1 : { $scale }   { $hint }
mask-cannot-draw = cannot draw the layer
mask-cannot-draw-full = cannot draw the layer in full

## network/errands.rs

errand-pausing = Pausing
errand-resuming = Resuming
errand-stopping = Stopping
errand-camera-on = Turning the camera on
errand-camera-off = Turning the camera off
errand-no-answer = the errand ended without an answer

## app.rs

app-cannot-build-preview = cannot build the layer preview

## job/hollow.rs

job-thread-stopped-hollowing = the hollowing thread stopped without finishing
job-hollow-stopping = Stopping after this model
job-hollow-progress = Hollowing model { $index } of { $total }
job-hollow-working = Hollowing...

## job/import.rs

job-thread-stopped-import = the import thread stopped without finishing
import-stage-reading = Reading
import-stage-repairing = Repairing
import-stage-indexing = Indexing
import-stage = { $stage } { $name }

## job/measure.rs

job-thread-stopped-measuring = the measuring thread stopped without finishing

## job/orient.rs

job-thread-stopped-orientation = the orientation thread stopped without finishing
job-orient-none = the search found no orientation

## job/place.rs

job-thread-stopped-support = the support thread stopped without finishing
job-placing = Placing supports, { $percent } %
job-cutting-plate = Cutting the plate...

## job/preview.rs

job-thread-stopped-preview = the preview thread stopped without finishing

## job/relief.rs

job-thread-stopped-relief = the relief thread stopped without finishing
job-relief-no-texture = a model has no texture to press

## job/send.rs

job-thread-stopped-network = the network thread stopped without finishing
job-send-starting = Starting { $printer }
job-send-connecting = Connecting to { $printer }

## job/trapped.rs

job-thread-stopped-drainage = the drainage thread stopped without finishing

## job/repair.rs

job-thread-stopped-repair = the thread repairing { $name } stopped without finishing
job-repairing = Repairing { $name }

## arrange.rs

arrange-no-footprint = a model on the plate has no footprint to arrange
arrange-nothing = Nothing visible on the plate to arrange
arrange-done = Arranged { $count } models
arrange-short =
    { $count ->
        [one] Arranged the plate; { $count } model found no room
       *[other] Arranged the plate; { $count } models found no room
    }

## drain.rs

drain-nothing-visible = nothing visible on the plate to check
drain-found =
    { $count ->
        [0] No trapped resin: everything can drain
        [one] { $count } pocket of trapped resin
       *[other] { $count } pockets of trapped resin
    }

## exchange.rs

exchange-cannot-import = cannot import { $path }
exchange-imported = Imported { $path }
exchange-choose-printer-file = choose the printer this file was cut for first
exchange-choose-printer-resin = choose the printer this resin was set up for first

## panels/find.rs

find-nothing = Nothing goes by that name

## panels/inspector/scope.rs

scope-applies-to = Applies to
scope-plate-choice = Plate
scope-model-choice = Model
scope-pick-one = Pick one model to give it numbers of its own.
scope-own = These numbers are this model's alone.
scope-keeping =
    { $count ->
        [one] { $count } model keeps numbers of its own.
       *[other] { $count } models keep numbers of their own.
    }

## panels/inspector/select.rs

select-selection = Selection
select-on-plate = { $count } on the plate
select-how = Click a model to pick it. Cmd-click adds one to the selection and shift-click takes everything between, and every tool then works on what is picked.

## panels/plate_strip.rs

plates-gather = Move the selection here
plates-remove = Remove with what is on it

## panels/printer_card/history.rs

history-none = It remembers no prints.
history-read-again = Read again
history-at-layer-of = at layer { $layer } of { $total }
history-at-layer = at layer { $layer }
history-finished = Finished { $when }
history-stopped = Stopped { $at }, { $when }
history-failed = Failed { $at }, { $when }

## panels/printer_card/ring.rs

ring-printers = Printers on the network
ring-open = Open this printer

## panels/printer_card/sheet.rs

sheet-recent-prints = Recent prints
sheet-firmware = firmware { $version }

## panels/settings/machines/library.rs

library-all-brands = All brands
library-custom = Custom printer
library-search = Search every machine
library-machines =
    { $count ->
        [one] { $count } machine
       *[other] { $count } machines
    }

## panels/stage.rs

stage-empty = No model on the plate
stage-empty-hint = Open a model or a project, or drop one into the window.
stage-open = Open...

## plate.rs

plate-select-printer = Select a printer

## preview/job.rs

preview-nothing-visible = nothing visible on the plate to preview
preview-cannot-cut = cannot cut the layers being previewed

## relief.rs

relief-no-pixels = the picture has no pixels

## web/opfs.rs

web-no-storage = this browser has no private file storage

## profiles.rs

picker-no-machine = No machine yet. Add one in Machine and resin.
picker-no-resin = No resins on this printer yet. Add them in Machine and resin.
picker-edit-resin = Edit this resin...
