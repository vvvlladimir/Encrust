# What Encrust can do

A plain-language list of every feature, with what you can change and what you cannot.
Command-line flags and worked examples are in [`cli.md`](cli.md); how each part works is in
[`design/`](design/); why it works that way is in [`decisions/`](decisions/).

Encrust is a free, open-source slicer for MSLA and SLA resin printers. It takes a 3D model
and turns it into the file your printer prints. Everything runs on your own computer: no
account, no cloud, nothing uploaded.

**Status: alpha.** Every step works end to end, from opening a model to starting the print,
but only one printer, the ELEGOO Saturn 4 Ultra, has printed a file Encrust wrote.

---

## Where it runs

| | What you get | What you don't |
|---|---|---|
| **Desktop window** | Windows 10/11 (installer, or the Microsoft Store), macOS 12+ (Apple silicon and Intel), Linux (AppImage, `.deb`). Every feature. | Downloaded builds are not code-signed yet, so the first launch needs one extra step; the Store copy does not. Needs a GPU with Vulkan, Metal or DirectX 12. |
| **Browser** | The same core, at encrust.app/app. Nothing to install, keeps working offline after the first visit, and the model never leaves your machine. | Needs WebGPU (a recent Chrome or Edge). It cannot send to a printer: the sliced file is downloaded instead. No self-update. The section cut is drawn without its cap. It trails the desktop build. |
| **Command line** (`encrust`) | The same pipeline with no window, for scripts and batches. | No painting, no manual supports. Anything you would click on a model is done in the window and saved as a project, which the command line can then slice. |

---

## Opening files

**You can:**
- Open **STL, OBJ and 3MF** models. OBJ and 3MF keep their UVs and one texture per
  material, which the Relief tool uses.
- Open an **`.encrust` project**. It opens exactly as it was saved, with its supports and
  cavities already built, so nothing is recalculated.
- Open **any sliced file** Encrust can read, whoever made it, as a model: its layers are
  built back into a mesh that stands where it stood on the plate, so you can move it,
  support it again or cut it for another printer. Its supports and raft come back as part
  of the model, edges are rounded by about a layer height (0.05 mm at the finest), and layer steps stay. A `.sl1` is slow
  to open (a minute for a full 16K stack). A file
  with the wrong extension is recognised by its first bytes.
- Drag and drop onto the window, or use the **Recent** list, which shows a picture of each
  file.
- Start from one of **four built-in test prints**: detail test, exposure check, surface
  finish and fit gauge.

**You cannot:**
- Open other 3D formats (STEP, PLY, AMF and so on).
- Open a project saved by version 3 or older of the project format.

---

## Fixing a model (Repair)

**You can:**
- See what the import found: holes, open edges, non-manifold edges, degenerate faces and
  faces wound the wrong way.
- Weld vertices together, with a tolerance you set.
- Flip faces wound the wrong way, and drop faces that enclose nothing.
- Close holes, but only when you ask for it.

**Guarantees:** a repair never makes a model worse than it was. An open model is never
capped silently.

---

## Placing models on the plate

**Position tool:**
- Move, rotate and scale with typed numbers or the on-screen gizmo, in global or local axes.
- Scale uniformly or per axis, keep the proportions, scale to fit the build volume, or go
  back to the imported size.
- **Mirror** the model, or mirror a copy beside it.
- **Lay on face:** click a face and the model lies on it.
- Put a model back in the middle of the plate, back on the plate, or lift it off by a set
  height, typed or a millimetre a press of − or +. Auto-orient keeps that height for a pose
  on supports; nought, or **On the plate**, hands it back to the support profile's lift.

**Auto-orient:**
- Searches for the best angle for one of four presets: **fewest supports**, **least
  peel**, **shortest print** or **auto**, picked in Position beside **Auto-orient**; the
  plate-wide **Auto-orient plate** uses the same preset. The preset resets to auto when
  the window reopens.
- **Auto** balances the other three for each model. It never lets one of supports, pull
  or print time go far worse for the sake of the others. Where some pose is about as good
  as the best on all three, it takes that one. Otherwise it takes the pose where each
  gives up the smallest, most even share of its best. Like least peel, it never takes a
  suction cup or a pull past the warning.
- When there are other good poses, pressing **Auto-orient** again with nothing changed
  stands the model the next of them, round to the first, without searching again. A
  change to the model, its supports, its lift, the preset, the printer or the resin
  searches again. With a printer chosen, the notice shows each pose's supports, pull and
  height, green where it beats the pose before and red where it loses. Auto offers every
  trade that no other pose beats on all three; a single goal offers the poses it reads as
  just as good.
- You can cap the tilt (for example "no more than 30° from how it stands now"). A cap of 0
  leaves the model as it is.
- It never picks an angle worse than the current one.
- With a printer chosen, it only picks a pose that fits the build volume, supports and
  all, if any pose does. It turns a part about the vertical, even across the plate's
  diagonal, when that is the only way it fits.
- A turned model stays over the spot it stood on. It is slid only as far as needed to
  keep it on the plate.
- With a printer chosen, it tries its best few poses with their supports and counts the
  islands each would leave at the printer's own pixel and layer, down to specks a tenth of
  a millimetre across. A pose standing on the plate with something hanging off it is
  turned down by every preset but fewest supports.
- **Fewest supports** picks the pose with the least resin in its supports, raft included,
  whichever way up that is — strictly the least, however small the saving. It ignores
  suction cups — drain them with **Drain**, or use **least peel**, which avoids them — and
  islands, takes supports with no room to stand as they come, and never picks a pose that
  would not fit the printer or that would balance on a point.
- **Least peel** keeps the hardest layer's pull on the film low, without suction cups or
  anything left hanging. Once every layer pulls under 10 N — a third of the 30 N warning —
  it has nothing more to win, and takes the pose with the least support among those, so
  a small model is not tilted or flipped for a fraction of a newton. The newtons are an
  estimate good to about a factor of three, not a measurement of your printer.
- **Shortest print** takes the pose with the fewest layers — the model's height plus its
  lift onto supports, since a layer takes as long whatever its area — that seals no cup and
  leaves nothing hanging. It ignores the peel warning and the support it takes: a flat
  part lies flat, a figure on its side. Poses within 2 % and a millimetre of the shortest
  count as just as fast, so a model is not turned for less.
- It either leaves a model standing on the plate, with supports only under what hangs
  over it, or lifts it onto supports by the support profile's **Lift** — whichever the
  goal prefers. Your raft setting is counted either way.

**Many models:**
- **Arrange the plate** automatically, largest model first.
- **Duplicate**, or lay copies out as an **array** of rows and columns with a gap you set.
  A copy is cut and hollowed once, not once per copy.
- **Several plates in one project.** Move models between plates, duplicate a plate, slice
  one plate or all of them at once.

**Measure tool:**
- Distance between two points, angle, or the gap between two flat faces.
- Snap to a corner, an edge or the exact spot (hold Alt for free placement). It can also show
  dX, dY and dZ in the view.
- Two faces that are not parallel have no single gap, and the tool says so.

---

## Changing the shape

### Cut
- Cut a model with a plane. Position the plane by height from the plate or by an offset
  from the centre of mass, or click to place it on the model.
- Add **alignment pins** (pegs and sockets, with diameter and length you set) so the halves
  glue back true.
- Lay each half flat on its cut face, or split a model into its separate parts.
- A piece the cut leaves broken is repaired automatically. Undo puts the original back.

### Relief (textures pressed into the surface)
- Use the model's own texture, or **any picture** tiled over the model from three sides.
- Set the depth (a negative depth sinks the picture in), the tile size, the angle and the
  precision.
- Using the model's own texture needs a model with UVs (OBJ or 3MF).

### Hollow
- Hollow a model with a wall thickness and precision you choose.
- **Inside** grows the wall inward and leaves the outside untouched. **Outside** grows it
  outward, which gives you a mould of the model.
- **Infill** inside the cavity: **Hive** (hexagons, the stiffest for its weight), **Grid**
  (square tubes) or **Scaffold** (struts on all three axes, the only one that takes a load
  from the side), or leave the cavity empty. Set the cell size and density.
- **Keep solid:** click spots on the model where the wall must stay solid (blockers), and
  set their size.
- Each model can keep its own hollow settings, and you can save settings as named presets.
- Undo with **Make solid again**.

### Drain
- **Drill** a hole, with size, depth and taper, wherever you click.
- **Channel:** dig a tube of the hole size through a path of points, open only at its two
  ends.
- **Check drainage:** Encrust follows the air through every layer and finds every pocket
  of resin with no way out. Pockets are painted in the view, and you can see through the
  model to find them.
- **Drill a hole into each** pocket automatically, at its lowest point.
- Drain settings can be saved as presets too.

---

## Supports

### Automatic
- **Grow the supports** for the whole plate, one model or only the parts you picked.
- Supports go only under what actually hangs over nothing, stay out of the model, and each
  one holds only the part it stands under.
- Nearby tips merge into shared **branching trunks**. Trunks are cones sized by their
  height and load, and nothing carrying a load leans more than 45°.
- A tip that has no room leans until it finds some. A support that still has no room is
  reported and left out of the print, never forced into the model.
- **Lift off the plate** stands the picked model that high, a millimetre a press of − or
  +, so supports have room. It is the same number in Position and in Supports.

### Manual
- **Place** a support with a click; Alt-click removes it.
- **Paint** areas with a brush where supports must go, then **Fill** them at a spacing you
  set. You can fill the inside of an area or only its rim, filtered by surface angle.
- **Block** areas where no support may go.
- Click a standing support to select it, then rebuild, remove or move it to another group.

### Groups and profiles
- Supports belong to **groups**. Each group is built to its own profile, so one model can
  have heavy supports underneath and light ones on its details.
- A thin support may stand on a thicker one from another group.
- Bundled profiles: **light**, **medium**, **heavy**. Duplicate one, edit it, save it under
  a new name, export it or import it. A bundled profile cannot be deleted.
- Each model remembers the profile it was edited with.

### What a profile lets you shape
- **Tip:** a plane or a sphere contact, with its diameter, contact spacing, how deep it
  bites into the part, and clearance.
- **Body:** diameter, overhang angle, branch angle, branching on or off, merge distance,
  maximum length, standing on the model on or off.
- **Small pillar:** a separate thin shape for short supports.
- **Base:** the pad's shape (round, cube, prism or tapered), its size, flare and wall slope.
- **Raft:** **Skates** (a saucer-shaped slab), **Grid** (rim and bars) or **Connected** (a
  pad under each foot, joined by strips), drawn to the model's outline (hull) or a
  rectangle, with margin and thickness. The raft is the plate's, whichever model is picked,
  and is kept when the window reopens. A model a project gave a raft of its own keeps it
  until **Use the plate's raft**.
- **Bracing:** cross braces between tall trunks, where the first brace goes and how often
  they repeat.

---

## Print settings

### Layers
- **Layer height.** When you change it, every exposure is moved along the resin's curve
  automatically. The old value is shown and can be restored with one press.
- **Adaptive layer height:** thick layers on steep walls, thin ones on gentle slopes, driven
  by a cusp target, with a thinnest and thickest layer.
  *Limit:* the printer must support variable layer height, and **no bundled profile claims
  it**, so you need a printer profile of your own with it turned on.
- **Several cutting planes per layer** for finer detail in each mask.

### Exposure
- Normal and bottom exposure, number of bottom layers, transition layers between them
  (linear).
- **Exposure bands:** a different exposure over a height range, as many bands as you like.
  The bottom layers always keep the resin's own exposure.
  *Limit:* some printers read only the file's header, so their exposure cannot change with
  height; Encrust says so. An `.sl1` file has one exposure and one layer height for the
  whole stack, so bands and adaptive layers are refused for it.

### Edge smoothing (anti-aliasing)
- Ready-made looks: **Sharp**, **Balanced**, **Soft**, **Crisp text**, **Organic curves**,
  or raw masks.
- Or set it by hand: grey levels, edge blur, grey floor, rounded edge grey.
- How much grey reaches the printer depends on its format (see [File formats](#file-formats)).

### Motion and waits
- Lift distance and speed, retract speed, separate values for the bottom layers.
- A setting the printer runs itself is not offered. On the ELEGOO Saturn 4 Ultra the vat
  tilts to separate a layer, so there are no lift or retract settings: the file says what the
  vendor's own files say, and the print time counts no lift.
- Wait before lift, after lift and after retract, light-off delay.
- Light power, and bottom light power where the printer supports it.

### Shrinkage compensation
- Scale each part to make up for resin shrinkage. Each part is scaled about its own
  footprint, so it does not slide its neighbours or lift off the plate. Supports are
  corrected together with the part they hold.
- Tolerance: move walls in or out, separately for the bottom layers and optionally every
  second layer.
- **Two calculators:** work out shrinkage from a measured test part (drawn vs. actual), and
  the extra time per layer from a finished print (predicted vs. clock).
- A compensation group can be switched off without losing its numbers.
- Compensation belongs to a resin on a machine; there is no per-job switch.

---

## Checking the print before it prints

The **Check** tool reads the layers that will really be cured, after supports, hollowing and
smoothing, not the original model.

**It finds:**
- **Islands:** parts that start in mid-air with nothing under them.
- **Suction cups:** closed hollows that pull on the film.
- **Thin sections and weak necks:** places too thin to hold.
- **Hard peel:** the layers that pull the film hardest, the widest step and the levers. A
  section's pull follows its area, less for a thin strip than for a disc of the same area.

**It lets you:**
- Jump to the layer of each issue.
- **Support every island** in one press.
- **Remove islands** from the file, then step by step remove whatever was left hanging.
  Put them back if you change your mind.
- **Drill the suction cups.**

**Estimates:** print time, resin by volume and weight, and cost if the resin has a price.
Resin is counted from what is actually cured.

---

## Viewing

- Lit, shadowed 3D view with a view cube, orthographic or perspective, and a plate grid.
- **See through models** (x-ray) to find cavities and trapped resin.
- **Section slider:** cut through the model at any height, capped so you can see what is
  solid.
- **Layer view:** the mask of each layer, the model and the layer side by side, or a range
  of layers. Type a layer number to jump to it.
- Ghost previews of hollowing, drains and supports before they are built.
- Heavy models are drawn at a level of detail the graphics card can handle.

---

## File formats

Every file Encrust writes is read back and checked. The output extension picks the format.

| Format | Printers | Grey on edges | Notes |
|---|---|---|---|
| `.goo` | Elegoo, Phrozen | yes | |
| `.ctb` v4, v5 | Elegoo, Creality, Phrozen, EPAX, Peopoly, QIDI, UniFormation | yes | |
| `.ctb` encrypted | the newer Chitu boards that take nothing else | yes | Written when the printer profile says `output = "ctb-encrypted"`; no shipped profile does yet. Read whoever wrote it. |
| `.cbddlp`, `.photon` | older Chitu-board printers | yes, in 8 passes | 8 times the layer data. Warns above 1440 × 2560 px. |
| `.pwmx` and the other Anycubic extensions | Anycubic | 4 bits | Edges dimmer than 1/16 are lost. |
| `.sl1`, `.sl1s` | Prusa | full 8 bits | One exposure and one layer height for the whole print; the bottom layers fade to it from the first one rather than holding. |
| `.zip` (PNG layers + G-code) | Phrozen, Wanhao, Kelant | full 8 bits | Per-layer exposure and height are kept. No second lift or acceleration. |
| `.cxdlp` v3, v4 | Creality | 8 bits (v3), 7 bits (v4) | |
| `.svgx` | FlashForge, Voxelab | none (vector polygons) | Warns above 3840 × 2400 px. |
| `.cws` | Nova3D | full 8 bits | Only the plain variant is written. |
| PNG folder | any | full 8 bits | No extension on the output name. |

Encrust also opens the `.pws` of the original Photon and Photon S, which it does not write.

**Convert** a sliced file to another format for another printer.
*Limits:* the masks are copied, never resampled, so the new printer's screen must have the
same size in pixels. A stack with varying layer heights cannot be converted yet, and the
converted file's preview pictures are blank.

The window offers only the machines whose format Encrust can actually write.

---

## Printers and profiles

**First setup:** while the plate has no printer or no resin, a notice at the foot of the
window says which, with **Set up** to open **Machine and resin**. There, an empty list of
printers offers **Add a machine** and **Import a machine**, and a printer with no resin
offers **Add a resin** and **Import a resin**. A file dropped on the window is imported
rather than opened: as a resin while a printer's resins are in view, or when it is a sliced
file or a resin profile, and as a printer otherwise. The first printer and resin you add
are the ones the plate prints on, with no second click.

**Printers:** 100 bundled profiles from 14 makers: Anycubic, Elegoo, Creality, Phrozen,
EPAX, FlashForge, Nova3D, Peopoly, QIDI, Wanhao, Prusa, Kelant, UniFormation, Voxelab.
Search the library by name or brand, or add a **custom printer**.

**What a printer profile holds:** name and maker, screen size in pixels, lit area in mm,
build volume, mirror across X or Y, grey floor, output format and version, firmware
abilities (per-layer settings, variable layer height), and the printers on your network it
sends to.

**Resins:** **none are bundled**, on purpose. A correct exposure depends on your machine and
your bottle, so you create the first resin yourself. A resin holds exposure, bottom layers,
transition, light power, lifts, retracts, waits, density, price, colour and compensation.
One resin can be tuned differently for each printer. A resin taken from a printer is kept
until you set it up on another one.

**Take a resin from a sliced file:** in **Machine and resin**, press **Import** on a
printer's resins (**Import a resin** on a printer with none yet) and pick a file another slicer wrote for it; the resin is saved on that
printer. You get what the
file states: exposure and bottom layers always, and transition, waits, lifts, speeds and
light power where the format holds them (`.goo` holds all of them, `.sl1` only the
transition). Anything
else keeps our defaults, and so does a lift the file states as a placeholder, as files for a
tilting vat do. Colour, price and compensation are not in sliced files.

**Take a resin from another program's profile:** import the resin profile it exports, a
`.cfgx` or an `.ini`, the same way; it goes on the printer you have chosen, or else on the
one the profile names. A `.cfgx` brings everything its resin holds: exposure, layers,
waits, lifts, light power, compensation, colour, density and price. An `.ini` brings
exposure, the faded layers, the XYZ correction, the elephant-foot compensation, colour,
density and price per litre; it holds no lifts or light power for other machines. An
encrypted profile, and a bundle of several materials in one file, cannot be read.

**Flexibility:**
- Every profile (printer, resin, support, hollow and drain presets) is a plain TOML text
  file you can open and edit. Unknown keys are refused, so a typo fails loudly instead of
  being ignored.
- Your own profiles live in a folder of their own, layered over the bundled ones. Open it
  from Settings.
- Export and import any profile as a file. An import never overwrites one you already have.
- Profile forms have undo.
- A project names its printer and resin by id, so it opens with your own profiles.

---

## Sending to a printer

| | Elegoo and Chitu boards (SDCP v3 and v1) | Prusa (PrusaLink) |
|---|---|---|
| Find on the network | automatic discovery, or add by IP address | added by host, not discovered |
| Send a file | yes, with a progress bar | yes, as one upload |
| Start the print | yes | yes |
| Pause, resume, stop | yes | yes |
| Live status (layer, progress) | yes | yes |
| Last prints, film wear | yes | no |
| Camera | turns it on and gives you the RTSP address | no |

- Printer cards in the window show each printer's status, a picture of the plate, and its
  controls. Stop needs a second press, because a stopped print cannot be resumed.
- Slice and send in one press, or slice now and start later.
- A Prusa login is kept in the window's settings, never in the profile.

**You cannot:** send from the browser build, or watch the camera inside Encrust (open the
address in a video player).

---

## Projects

- Save the whole job as an `.encrust` project: every plate, model, support tree, cavity and
  setting exactly as it stands.
- It reopens instantly, without rebuilding anything, and the command line can slice it
  directly.

---

## Working in the window

- **Find (⌘K):** search every tool, action and model on the plate by name.
- **Keyboard:** digits 1 to 0 pick the first ten tools, there is a shortcut sheet, Tab walks
  every control, and every control has a name for screen readers.
- **Undo and redo** across the plate and the settings forms.
- Tool values are remembered between runs.
- **Updates:** checks once a day (can be turned off), downloads, verifies the signature and
  installs only when you press it. You can skip a version. A copy from the Microsoft Store
  is updated by the Store instead: it says when the Store has a new version, and its button
  opens Encrust in the Store app to update.
- **Bug report:** builds a Markdown report (what happened, how to reproduce it, settings,
  profiles) and gives it to you to copy or save. Encrust never sends it itself, and network
  addresses are removed from it.
- Choose where sliced files are saved and whether the window opens on the start page.
- **Language:** the window follows the system's or the browser's first language, or the one
  you pick in Settings → General. A system language with no translation shows the window in
  English, and Settings says so. English and Russian are complete; a message a translation
  lacks shows in English. The test prints keep English file names in every
  language. The command line stays English.

---

## Command line

The whole pipeline from a terminal. See [`cli.md`](cli.md) for every flag.

| Command | What it does |
|---|---|
| `slice` | One model, several models on one plate, a plate file (TOML) or a project, to a printer file or PNGs. Orient, support, hollow, drain, relief, exposure bands, adaptive layers and island removal are all flags. |
| `batch` | A whole folder of models, several at once, with a JSON report for each. A model that fails does not stop the rest. |
| `estimate` | Time, resin, weight, cost and risky layers, without writing a file. |
| `inspect` | What is wrong with a model and whether it fits. |
| `info` | What a sliced file says, checked layer by layer. Export one layer as PNG. |
| `convert` | A sliced file to another format. |
| `profiles` | List, show, export and import profiles. |
| `printer` | Discover, status and send, with optional start. |
| `completions` | Shell completion for bash, zsh, fish, elvish, PowerShell. |

**For scripts:** `--json` output with a fixed schema, clear exit codes, `--strict` (fail
and leave nothing on disk if a model has defects), a config file for flags you always use,
progress bars that turn off when not on a terminal, and Ctrl-C that stops cleanly and
removes the half-written file.

**You cannot** (from the command line): paint supports, place them by hand, or set blockers.
Do that in the window, save a project, then slice the project.

---

## Known limits at a glance

- Only the **ELEGOO Saturn 4 Ultra** is confirmed by a real print. Start every other printer
  with a small calibration piece and check the build area and mirroring.
- **No bundled resins.** Calibrate your own exposure.
- **Encrypted `.ctb`** is written only for a printer whose profile asks for it, and no
  shipped profile does until a machine is confirmed by a print.
- **Adaptive layer height** needs a custom printer profile; no bundled one allows it.
- **Convert** cannot carry varying layer heights or the preview pictures.
- **Browser build:** no network sending, no updates, uncapped section cut.
- **Camera:** the address only, no video inside Encrust.
- **Translations:** messages from inside the slicing engine (a file that cannot be read, a
  format it does not know) stay in English under a translated first line.
- It is alpha: before a long print, compare the output with a slicer you already trust.
