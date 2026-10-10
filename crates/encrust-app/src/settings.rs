//! The Settings screen's state: which page is open, what is picked on it, and the profile
//! being edited, which is written back as it changes. See `docs/design/profiles.md`.

use std::collections::BTreeMap;

use printer_profiles::{
    Catalogue, Entry, Kind, MaterialProfile, PrinterProfile, PrinterTuning, ProfileError, Source,
    SupportProfile,
};

use crate::slicing::Rescaled;

/// A page of the Settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Supports,
    /// Whether to look for a new release, and the one found.
    Updates,
}

impl Section {
    #[cfg(not(target_arch = "wasm32"))]
    pub const ALL: [Self; 2] = [Self::Supports, Self::Updates];

    /// A page loads the latest release every time it is opened, so it has nothing to update.
    #[cfg(target_arch = "wasm32")]
    pub const ALL: [Self; 1] = [Self::Supports];

    pub fn label(self) -> &'static str {
        match self {
            Self::Supports => "Supports",
            Self::Updates => "Updates",
        }
    }
}

/// A tab of the Machine and resin window, over the machine picked down its left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// The resins set up on the machine, as a table.
    #[default]
    Resins,
    /// The machine's own profile: panel, volume, output and firmware.
    Machine,
    /// What the machine takes a file over, and which one on the network it is.
    Network,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::Resins, Self::Machine, Self::Network];

    pub fn label(self) -> &'static str {
        match self {
            Self::Resins => "Resins",
            Self::Machine => "Machine",
            Self::Network => "Network",
        }
    }
}

/// What the Machine and resin window has open: a printer, or one of the resins set up on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Printer(String),
    Resin { printer: String, resin: String },
}

impl Node {
    /// The printer the node is, or the one it is a resin of.
    pub fn printer(&self) -> &str {
        match self {
            Self::Printer(printer) | Self::Resin { printer, .. } => printer,
        }
    }
}

/// One profile open for editing, and what of it was last written.
pub struct Draft<T> {
    pub id: String,
    pub values: T,
    saved: T,
    /// What this build ships under the same id, when it ships anything. A copy taken out
    /// of the library by an older release stands over it and says nothing; see ADR 0196.
    shipped: Option<T>,
}

impl<T: Clone + PartialEq> Draft<T> {
    fn new(id: String, values: T) -> Self {
        Self {
            id,
            saved: values.clone(),
            values,
            shipped: None,
        }
    }

    /// The same, over the profile the build ships under that id.
    fn over_shipped(id: String, values: T, shipped: Option<T>) -> Self {
        Self {
            shipped,
            ..Self::new(id, values)
        }
    }

    /// Whether the form holds something not yet written.
    pub fn is_dirty(&self) -> bool {
        self.values != self.saved
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.values.clone();
    }

    /// The profile the build ships under this id, while the user's copy differs from it.
    pub fn shipped(&self) -> Option<&T> {
        self.shipped
            .as_ref()
            .filter(|shipped| **shipped != self.saved)
    }
}

/// A resin as one printer needs it, beside the resin with every printer's table.
pub struct ResinDraft {
    pub printer: String,
    pub draft: Draft<MaterialProfile>,
    pub base: MaterialProfile,
    /// The exposure a change of the measured height carried along, as it was. A profile
    /// has no bands, so the bands of it stay empty.
    carried: Option<Rescaled>,
}

impl ResinDraft {
    fn new(printer: String, id: String, base: MaterialProfile) -> Self {
        Self {
            draft: Draft::new(id, base.starting_point(&printer)),
            printer,
            base,
            carried: None,
        }
    }

    /// Moves the height the exposure is measured at, carrying the exposure along the
    /// working curve so that the profile still cures a layer as deep; see
    /// `docs/decisions/0128`.
    pub fn set_layer_height(&mut self, layer_height_mm: f32) {
        let values = &mut self.draft.values;
        let from_mm = values.layer_height_mm;
        if (layer_height_mm - from_mm).abs() < f32::EPSILON {
            return;
        }
        let before = self.carried.take().unwrap_or(Rescaled {
            from_mm,
            exposure_s: values.exposure_s,
            bands: Vec::new(),
        });
        if (layer_height_mm - before.from_mm).abs() < f32::EPSILON {
            values.layer_height_mm = layer_height_mm;
            values.exposure_s = before.exposure_s;
            return;
        }
        *values = values.rescaled_to(layer_height_mm);
        self.carried = Some(before);
    }

    /// What the last change of the measured height carried along, while untouched.
    pub fn carried(&self) -> Option<&Rescaled> {
        self.carried.as_ref()
    }

    /// Puts back the exposure the last change of height carried along.
    pub fn revert_exposure(&mut self) {
        if let Some(before) = self.carried.take() {
            self.draft.values.exposure_s = before.exposure_s;
        }
    }

    /// The exposure was set by hand.
    pub fn exposure_edited(&mut self) {
        self.carried = None;
    }

    /// The resin to write: the edits become this printer's table and every other printer
    /// keeps its own. What the resin is and what it costs belong to the resin itself.
    pub fn to_saved(&self) -> MaterialProfile {
        let edited = &self.draft.values;
        let mut saved = self.base.clone();
        saved.name = edited.name.clone();
        saved.density_g_cm3 = edited.density_g_cm3;
        saved.details = edited.details.clone();
        let tuning = PrinterTuning::of_changes(&saved, edited);
        saved.printers.insert(self.printer.clone(), tuning);
        saved.last_printer = Some(self.printer.clone());
        saved
    }
}

/// Which compensation calculator is open over the resin form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Calculator {
    /// A measured part against the size it was drawn at.
    Shrinkage,
    /// One print's predicted time against the clock.
    LayerTime,
}

/// What a calculator has been typed into, kept while it is open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calculators {
    pub open: Option<Calculator>,
    /// The size a test part was drawn at, X, Y and Z, millimetres.
    pub nominal_mm: [f32; 3],
    /// The size it measured after printing and curing.
    pub printed_mm: [f32; 3],
    /// Hours, minutes and seconds the estimate gave.
    pub predicted: [f32; 3],
    /// Hours, minutes and seconds the machine took.
    pub actual: [f32; 3],
    pub layers: f32,
}

impl Default for Calculators {
    /// A 20 mm calibration cube, which is what most calibration prints are.
    fn default() -> Self {
        Self {
            open: None,
            nominal_mm: [20.0; 3],
            printed_mm: [20.0; 3],
            predicted: [0.0; 3],
            actual: [0.0; 3],
            layers: 0.0,
        }
    }
}

impl Calculators {
    /// Seconds a triple of hours, minutes and seconds comes to.
    pub fn seconds(clock: [f32; 3]) -> f32 {
        clock[0] * 3600.0 + clock[1] * 60.0 + clock[2]
    }
}

/// A deletion waiting to be answered for. The only one this screen takes back is a resin
/// added again from the pool, so each is asked first; see ADR 0196 and 0197.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deleting {
    /// The user's copy of a machine. Its resins stay in the pool.
    Printer(String),
    /// A resin off one printer. It waits in the pool unless nobody ever typed into it.
    ResinOff { printer: String, resin: String },
    /// A resin and its file, off every printer that has it.
    Resin(String),
}

/// What the machine library has open: a brand, and what its models are filtered by.
#[derive(Default)]
pub struct Library {
    pub brand: Option<String>,
    pub search: String,
}

/// What the Settings screen and the Machine and resin window are showing.
#[derive(Default)]
pub struct Settings {
    /// Whether the Settings screen stands in the plate's place.
    pub open: bool,
    pub section: Section,
    /// Whether the Machine and resin window is over the window.
    pub machines: bool,
    pub tab: Tab,
    /// The resin type the table is narrowed to, by the name on the bottle.
    pub kind: Option<String>,
    /// Whether the resin picked in the table is open in its form instead of the table.
    pub editing_resin: bool,
    pub node: Option<Node>,
    /// What the machine list is filtered by, as it is being typed.
    pub search: String,
    /// The machine library, while it stands in the tabs' place.
    pub library: Option<Library>,
    pub printer: Option<Draft<PrinterProfile>>,
    pub resin: Option<ResinDraft>,
    pub support: Option<Draft<SupportProfile>>,
    pub calculators: Calculators,
    /// The deletion the window is waiting for an answer on.
    pub confirm: Option<Deleting>,
}

impl Settings {
    /// Opens the Machine and resin window on what the plate is using: its resin, open in
    /// its form, when that is set up on its printer, the printer's resins otherwise.
    pub fn open_machines(
        &mut self,
        catalogue: &Catalogue,
        printer_id: Option<&str>,
        resin_id: Option<&str>,
    ) {
        self.machines = true;
        self.tab = Tab::Resins;
        self.kind = None;
        self.library = None;
        let printer = printer_id
            .filter(|id| catalogue.printer(id).is_ok())
            .map(str::to_owned)
            .or_else(|| {
                installed_printers(catalogue)
                    .next()
                    .map(|entry| entry.id.clone())
            });
        let Some(printer) = printer else {
            // Nothing installed yet, so the window opens on the only thing there is to do.
            self.library = Some(Library::default());
            return;
        };
        let resin = resin_id.filter(|id| {
            catalogue
                .resin(id)
                .is_ok_and(|entry| entry.profile.is_tuned_for(&printer))
        });
        self.editing_resin = resin.is_some();
        let node = match resin {
            Some(resin) => Node::Resin {
                printer,
                resin: resin.to_owned(),
            },
            None => Node::Printer(printer),
        };
        self.pick(catalogue, node);
    }

    /// Closes the Machine and resin window, and the question and calculator over it.
    pub fn close_machines(&mut self) {
        self.machines = false;
        self.editing_resin = false;
        self.library = None;
        self.confirm = None;
        self.calculators.open = None;
    }

    /// Opens the screen on the support profiles, at the one `profile` was taken from.
    pub fn open_supports(&mut self, catalogue: &Catalogue, profile: &SupportProfile) {
        self.open = true;
        self.section = Section::Supports;
        let entry = catalogue
            .supports()
            .find(|entry| entry.profile == *profile)
            .or_else(|| catalogue.supports().next());
        if let Some(entry) = entry {
            self.support = Some(Draft::new(entry.id.clone(), entry.profile.clone()));
        }
    }

    /// Loads a support profile into the form.
    pub fn pick_support(&mut self, catalogue: &Catalogue, id: &str) {
        self.support = catalogue
            .support(id)
            .ok()
            .map(|entry| Draft::new(id.to_owned(), entry.profile.clone()));
    }

    /// Opens a printer or one of its resins in the form.
    pub fn pick(&mut self, catalogue: &Catalogue, node: Node) {
        if self.node.as_ref().map(Node::printer) != Some(node.printer()) {
            self.kind = None;
        }
        if matches!(node, Node::Printer(_)) {
            self.editing_resin = false;
        }
        self.printer = catalogue.printer(node.printer()).ok().map(|entry| {
            Draft::over_shipped(
                entry.id.clone(),
                entry.profile.clone(),
                catalogue.shipped_printer(&entry.id),
            )
        });
        self.resin = match &node {
            Node::Printer(_) => None,
            Node::Resin { printer, resin } => catalogue.resin(resin).ok().map(|entry| {
                ResinDraft::new(printer.clone(), resin.clone(), entry.profile.clone())
            }),
        };
        self.node = Some(node);
    }
}

/// How many machines the library holds, and by how many makers.
pub fn library_size(catalogue: &Catalogue) -> (usize, usize) {
    let makers: std::collections::BTreeSet<&str> = catalogue
        .printers()
        .map(|entry| entry.profile.manufacturer.as_str())
        .collect();
    (catalogue.printers().count(), makers.len())
}

/// The resins set up on `printer`, in the catalogue's order.
pub fn resins_of<'a>(
    catalogue: &'a Catalogue,
    printer: &'a str,
) -> impl Iterator<Item = (&'a str, &'a MaterialProfile)> {
    catalogue
        .resins()
        .filter(move |entry| installed(&entry.source) && entry.profile.is_tuned_for(printer))
        .map(|entry| (entry.id.as_str(), &entry.profile))
}

/// Whether a profile is one the user has, rather than one the catalogue merely offers.
/// The window lists what is installed; the shipped profiles are the library behind the
/// `+` and the Add resin menu. See `docs/decisions/0158`.
pub fn installed(source: &Source) -> bool {
    matches!(source, Source::User(_))
}

/// The machines the user installed, in catalogue order.
pub fn installed_printers(catalogue: &Catalogue) -> impl Iterator<Item = &Entry<PrinterProfile>> {
    catalogue
        .printers()
        .filter(|entry| installed(&entry.source))
}

/// Every resin the catalogue has that `printer` is not already set up with: the shipped
/// ones as well, which is the only way they are reached.
pub fn pool_for<'a>(
    catalogue: &'a Catalogue,
    printer: &'a str,
) -> impl Iterator<Item = (&'a str, &'a MaterialProfile)> {
    catalogue
        .resins()
        .filter(move |entry| !installed(&entry.source) || !entry.profile.is_tuned_for(printer))
        .map(|entry| (entry.id.as_str(), &entry.profile))
}

/// Sets a resin from the pool up on `printer`, starting from the numbers of the printer
/// it was last tuned on.
pub fn add_resin(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
) -> Result<(), ProfileError> {
    let mut base = catalogue.resin(resin)?.profile.clone();
    let start = base.starting_point(printer);
    let tuning = PrinterTuning::of_changes(&base, &start);
    base.printers.insert(printer.to_owned(), tuning);
    base.last_printer = Some(printer.to_owned());
    catalogue.save_resin(resin, &base).map(drop)
}

/// A new resin on `printer` alone, with the stock numbers. Returns its id.
pub fn new_resin(catalogue: &mut Catalogue, printer: &str) -> Result<String, ProfileError> {
    save_as_own(catalogue, printer, stock_resin())
}

/// What **New resin** makes: the stock numbers under a name that says it is new.
fn stock_resin() -> MaterialProfile {
    MaterialProfile {
        name: "New resin".to_owned(),
        ..MaterialProfile::default()
    }
}

/// Whether nothing has been typed into a resin since it was made: the stock numbers, the
/// stock name, and no machine's table carrying a change.
pub fn is_untouched(resin: &MaterialProfile) -> bool {
    let bare = MaterialProfile {
        printers: BTreeMap::new(),
        last_printer: None,
        ..resin.clone()
    };
    bare == stock_resin()
        && resin
            .printers
            .values()
            .all(|tuning| *tuning == PrinterTuning::default())
}

/// A copy of a resin as `printer` has it, on that printer alone. Returns its id.
pub fn duplicate_resin(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
) -> Result<String, ProfileError> {
    let mut copy = catalogue.resin(resin)?.profile.for_printer(printer);
    copy.name = format!("{} copy", copy.name);
    save_as_own(catalogue, printer, copy)
}

/// Takes a resin off `printer`. It stays in the pool for any printer to take back, and is
/// deleted from there; one nobody ever typed into is not worth keeping, so taking that
/// off its last printer throws it away (ADR 0197).
pub fn take_resin_off(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
) -> Result<(), ProfileError> {
    match is_thrown_away_with_the_printer(catalogue, printer, resin)? {
        true => delete_resin(catalogue, resin),
        false => remove_resin(catalogue, printer, resin),
    }
}

/// Whether taking this resin off `printer` takes it away altogether rather than leaving
/// it in the pool, which is what the question before it has to say.
pub fn is_thrown_away_with_the_printer(
    catalogue: &Catalogue,
    printer: &str,
    resin: &str,
) -> Result<bool, ProfileError> {
    let profile = &catalogue.resin(resin)?.profile;
    let last = profile.printers.keys().all(|id| id == printer);
    Ok(last && is_untouched(profile))
}

/// Throws a resin away, off every printer that had it and out of the user's directory.
pub fn delete_resin(catalogue: &mut Catalogue, resin: &str) -> Result<(), ProfileError> {
    catalogue.forget_user_copy(Kind::Resin, resin)
}

/// The printers other than `printer` a resin carries numbers for, by name, which is what
/// a question about deleting it has to say out loud.
pub fn other_printers(
    catalogue: &Catalogue,
    printer: &str,
    resin: &str,
) -> Result<Vec<String>, ProfileError> {
    let profile = &catalogue.resin(resin)?.profile;
    Ok(profile
        .printers
        .keys()
        .filter(|id| id.as_str() != printer)
        .map(|id| match catalogue.printer(id) {
            Ok(entry) => format!("{} {}", entry.profile.manufacturer, entry.profile.name),
            Err(_) => id.clone(),
        })
        .collect())
}

/// Takes a resin off `printer`. It stays in the pool for any printer to take back.
pub fn remove_resin(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
) -> Result<(), ProfileError> {
    let mut base = catalogue.resin(resin)?.profile.clone();
    base.printers.remove(printer);
    if base.last_printer.as_deref() == Some(printer) {
        base.last_printer = base.printers.keys().next().cloned();
    }
    catalogue.save_resin(resin, &base).map(drop)
}

/// Renames a resin as `printer` lists it. A resin other printers use too is split off
/// under the new name, so theirs keeps its name. Returns the id the resin now has here.
pub fn rename_resin(
    catalogue: &mut Catalogue,
    printer: &str,
    resin: &str,
    name: &str,
) -> Result<String, ProfileError> {
    let mut base = catalogue.resin(resin)?.profile.clone();
    let shared = base.printers.keys().any(|other| other != printer);
    if !shared {
        base.name = name.to_owned();
        catalogue.save_resin(resin, &base)?;
        return Ok(resin.to_owned());
    }
    let mut own = base.for_printer(printer);
    own.name = name.to_owned();
    let id = save_as_own(catalogue, printer, own)?;
    remove_resin(catalogue, printer, resin)?;
    Ok(id)
}

/// Writes `values` as a new resin whose own numbers are this printer's, under the first
/// free id its name makes.
fn save_as_own(
    catalogue: &mut Catalogue,
    printer: &str,
    values: MaterialProfile,
) -> Result<String, ProfileError> {
    let resin = MaterialProfile {
        printers: BTreeMap::from([(printer.to_owned(), PrinterTuning::default())]),
        last_printer: Some(printer.to_owned()),
        ..values
    };
    let id = unique_id(&resin.name, |id| catalogue.resin(id).is_ok());
    catalogue.save_resin(&id, &resin)?;
    Ok(id)
}

/// A new printer with the stock numbers. Returns its id.
pub fn new_printer(catalogue: &mut Catalogue) -> Result<String, ProfileError> {
    let printer = PrinterProfile {
        name: "New printer".to_owned(),
        ..PrinterProfile::default()
    };
    let id = unique_id(&printer.name, |id| catalogue.printer(id).is_ok());
    catalogue.save_printer(&id, &printer)?;
    Ok(id)
}

/// A new support profile with the stock numbers, or a copy of `from`. Returns its id.
pub fn new_support(catalogue: &mut Catalogue, from: Option<&str>) -> Result<String, ProfileError> {
    let profile = match from {
        Some(id) => {
            let mut copy = catalogue.support(id)?.profile.clone();
            copy.name = format!("{} copy", copy.name);
            copy
        }
        None => SupportProfile {
            name: "New profile".to_owned(),
            ..SupportProfile::default()
        },
    };
    keep_support(catalogue, &profile)
}

/// Keeps `profile` as a support profile of its own. Returns its id.
pub fn keep_support(
    catalogue: &mut Catalogue,
    profile: &SupportProfile,
) -> Result<String, ProfileError> {
    let id = unique_id(&profile.name, |id| catalogue.support(id).is_ok());
    catalogue.save_support(&id, profile)?;
    Ok(id)
}

/// Whether a profile can be thrown away: one only the user ever had. A shipped one would
/// only come back.
pub fn can_delete(catalogue: &Catalogue, kind: Kind, id: &str) -> bool {
    !catalogue.is_shipped(kind, id)
}

/// The file stem `name` makes, numbered past the ones `taken` already claims.
pub fn unique_id(name: &str, taken: impl Fn(&str) -> bool) -> String {
    // A name in a script a file stem cannot hold still names a profile.
    let stem = match slug(name) {
        stem if stem.is_empty() => "profile".to_owned(),
        stem => stem,
    };
    let mut id = stem.clone();
    let mut next = 2;
    while taken(&id) {
        id = format!("{stem}-{next}");
        next += 1;
    }
    id
}

/// The file stem a name makes: lower-case letters and digits, one dash between words.
pub fn slug(name: &str) -> String {
    let mut stem = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            stem.push(c.to_ascii_lowercase());
        } else if !stem.is_empty() && !stem.ends_with('-') {
            stem.push('-');
        }
    }
    stem.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_a_resins_measured_height_carries_its_exposure_and_can_be_taken_back() {
        let resin = MaterialProfile {
            layer_height_mm: 0.05,
            exposure_s: 2.5,
            penetration_depth_mm: None,
            ..MaterialProfile::default()
        };
        let mut draft = ResinDraft::new(MARS.to_owned(), "grey".to_owned(), resin);
        draft.set_layer_height(0.1);
        assert!((draft.draft.values.exposure_s - 4.1218).abs() < 1e-3);
        assert!(
            draft.carried().is_some(),
            "the change is shown until touched"
        );

        draft.revert_exposure();
        assert_eq!(draft.draft.values.exposure_s, 2.5);
        assert!((draft.draft.values.layer_height_mm - 0.1).abs() < 1e-6);
        assert!(draft.carried().is_none());
    }

    const MARS: &str = "elegoo-mars-3-pro";
    const SATURN: &str = "elegoo-saturn-4-ultra";

    /// The shipped catalogue over a directory of its own, which every write lands in.
    fn writable(name: &str) -> Catalogue {
        let dir =
            std::env::temp_dir().join(format!("encrust-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Catalogue::with_root(&dir).expect("a missing directory reads as empty")
    }

    /// The same, with one resin the user measured on both machines: nothing ships an
    /// exposure, so a test that needs a resin makes one (ADR 0196).
    fn with_grey(name: &str) -> Catalogue {
        let mut catalogue = writable(name);
        let mut resin = MaterialProfile {
            name: "Standard grey".to_owned(),
            exposure_s: 2.6,
            ..MaterialProfile::default()
        };
        for (printer, exposure_s) in [(MARS, 3.2), (SATURN, 2.3)] {
            resin.printers.insert(
                printer.to_owned(),
                PrinterTuning {
                    exposure_s: Some(exposure_s),
                    ..PrinterTuning::default()
                },
            );
        }
        resin.last_printer = Some(MARS.to_owned());
        catalogue
            .save_resin("standard-grey", &resin)
            .expect("the directory is writable");
        catalogue
    }

    #[test]
    fn a_name_makes_a_file_stem() {
        assert_eq!(slug("Saturn 4 Ultra"), "saturn-4-ultra");
        assert_eq!(slug("  ABS-like (tough)! "), "abs-like-tough");
        assert_eq!(unique_id("Смола", |_| false), "profile");
        assert_eq!(
            unique_id("Grey", |id| id == "grey" || id == "grey-2"),
            "grey-3"
        );
    }

    #[test]
    fn a_resin_of_another_printer_is_in_the_pool_until_it_is_added_to_this_one() {
        let mut catalogue = writable("installed");
        assert!(
            resins_of(&catalogue, SATURN).next().is_none(),
            "a first run has no resin at all"
        );
        assert!(
            pool_for(&catalogue, SATURN).next().is_none(),
            "and nothing is shipped to put in the pool"
        );

        let mine = new_resin(&mut catalogue, MARS).expect("the directory is writable");
        assert!(
            pool_for(&catalogue, SATURN).any(|(id, _)| id == mine),
            "a resin measured on another machine is what the pool offers"
        );

        add_resin(&mut catalogue, SATURN, &mine).expect("the directory is writable");
        assert!(resins_of(&catalogue, SATURN).any(|(id, _)| id == mine));
        assert!(
            !pool_for(&catalogue, SATURN).any(|(id, _)| id == mine),
            "a resin on this printer is no longer offered for it"
        );
    }

    /// A resin somebody measured is theirs: off every printer it still waits in the pool,
    /// and the pool is where it is deleted from.
    #[test]
    fn a_measured_resin_off_its_last_printer_waits_in_the_pool() {
        let mut catalogue = with_grey("last-printer");
        for printer in [MARS, SATURN] {
            take_resin_off(&mut catalogue, printer, "standard-grey").expect("writable");
        }
        assert!(resins_of(&catalogue, MARS).next().is_none());
        assert!(
            pool_for(&catalogue, MARS).any(|(id, _)| id == "standard-grey"),
            "on no printer, so every printer is offered it"
        );

        add_resin(&mut catalogue, MARS, "standard-grey").expect("writable");
        assert!(resins_of(&catalogue, MARS).any(|(id, _)| id == "standard-grey"));
    }

    /// BUG-16: a resin made and never typed into is not worth a file, so taking it off
    /// the only printer it was ever on throws it away instead.
    #[test]
    fn a_resin_nobody_typed_into_goes_with_the_printer_it_was_made_on() {
        let mut catalogue = writable("untouched");
        let id = new_resin(&mut catalogue, MARS).expect("writable");
        assert!(is_thrown_away_with_the_printer(&catalogue, MARS, &id).expect("there"));

        take_resin_off(&mut catalogue, MARS, &id).expect("writable");
        assert!(
            catalogue.resin(&id).is_err(),
            "no file, nothing in the pool"
        );
    }

    #[test]
    fn one_number_typed_into_a_new_resin_keeps_it() {
        let mut catalogue = writable("touched");
        let id = new_resin(&mut catalogue, MARS).expect("writable");
        let mut edited = catalogue.resin(&id).expect("saved").profile.clone();
        edited.printers.insert(
            MARS.to_owned(),
            PrinterTuning {
                exposure_s: Some(2.9),
                ..PrinterTuning::default()
            },
        );
        catalogue.save_resin(&id, &edited).expect("writable");
        assert!(!is_thrown_away_with_the_printer(&catalogue, MARS, &id).expect("there"));

        take_resin_off(&mut catalogue, MARS, &id).expect("writable");
        assert!(
            pool_for(&catalogue, MARS).any(|(other, _)| other == id),
            "a measured exposure waits in the pool"
        );
    }

    /// What the question before a deletion has to say: who else loses the resin.
    #[test]
    fn the_printers_a_resin_is_shared_with_are_named() {
        let catalogue = with_grey("shared");
        let others = other_printers(&catalogue, MARS, "standard-grey").expect("there");
        assert_eq!(others, ["Elegoo Saturn 4 Ultra"]);
    }

    #[test]
    fn a_resin_deleted_from_the_pool_leaves_every_printer() {
        let mut catalogue = with_grey("delete-resin");
        delete_resin(&mut catalogue, "standard-grey").expect("writable");
        assert!(catalogue.resin("standard-grey").is_err());
        assert!(resins_of(&catalogue, MARS).next().is_none());
        assert!(resins_of(&catalogue, SATURN).next().is_none());
    }

    #[test]
    fn a_resin_added_to_a_second_printer_is_on_both() {
        let mut catalogue = with_grey("both");
        add_resin(&mut catalogue, SATURN, "standard-grey").expect("the directory is writable");
        assert!(resins_of(&catalogue, SATURN).any(|(id, _)| id == "standard-grey"));
        assert!(
            !pool_for(&catalogue, SATURN).any(|(id, _)| id == "standard-grey"),
            "a resin on this printer is no longer offered for it"
        );
    }

    #[test]
    fn the_window_opens_on_the_resin_the_plate_uses() {
        let catalogue = with_grey("open");
        let mut settings = Settings::default();
        settings.open_machines(&catalogue, Some(MARS), Some("standard-grey"));
        let resin = settings.resin.as_ref().expect("a resin is open");
        assert_eq!(resin.draft.id, "standard-grey");
        assert!(
            (resin.draft.values.exposure_s - 3.2).abs() < 1e-6,
            "the Mars 3 Pro's own exposure for this resin"
        );
    }

    #[test]
    fn an_edit_for_one_printer_leaves_every_other_alone() {
        let catalogue = with_grey("edit");
        let mut settings = Settings::default();
        settings.open_machines(&catalogue, Some(MARS), Some("standard-grey"));
        let resin = settings.resin.as_mut().expect("open");
        let saturn_before = resin.base.for_printer(SATURN).exposure_s;

        resin.draft.values.exposure_s = 4.0;
        assert!(resin.draft.is_dirty());
        let saved = resin.to_saved();
        assert!((saved.for_printer(MARS).exposure_s - 4.0).abs() < 1e-6);
        assert!((saved.for_printer(SATURN).exposure_s - saturn_before).abs() < 1e-6);
    }

    #[test]
    fn a_resin_taken_off_a_printer_stays_in_the_pool_and_comes_back() {
        let mut catalogue = with_grey("pool");
        remove_resin(&mut catalogue, MARS, "standard-grey").expect("writable");
        assert!(resins_of(&catalogue, MARS).all(|(id, _)| id != "standard-grey"));
        assert!(pool_for(&catalogue, MARS).any(|(id, _)| id == "standard-grey"));
        assert!(
            resins_of(&catalogue, SATURN).any(|(id, _)| id == "standard-grey"),
            "the other printers keep it"
        );

        add_resin(&mut catalogue, MARS, "standard-grey").expect("writable");
        assert!(resins_of(&catalogue, MARS).any(|(id, _)| id == "standard-grey"));
    }

    #[test]
    fn a_duplicate_belongs_to_the_printer_it_was_made_on() {
        let mut catalogue = with_grey("duplicate");
        let id = duplicate_resin(&mut catalogue, MARS, "standard-grey").expect("writable");
        assert_eq!(id, "standard-grey-copy");
        let copy = &catalogue.resin(&id).expect("saved").profile;
        assert!(copy.is_tuned_for(MARS) && !copy.is_tuned_for(SATURN));
        assert!(
            (copy.for_printer(MARS).exposure_s - 3.2).abs() < 1e-6,
            "the Mars numbers"
        );
    }

    #[test]
    fn renaming_a_shared_resin_splits_it_off_this_printer() {
        let mut catalogue = with_grey("rename");
        let id =
            rename_resin(&mut catalogue, MARS, "standard-grey", "Grey fast").expect("writable");
        assert_eq!(id, "grey-fast");
        let kept = &catalogue.resin("standard-grey").expect("kept").profile;
        assert_eq!(
            kept.name, "Standard grey",
            "the other printers keep the name"
        );
        assert!(!kept.is_tuned_for(MARS));

        let again = rename_resin(&mut catalogue, MARS, &id, "Grey faster").expect("writable");
        assert_eq!(
            again, id,
            "a resin only this printer has is renamed where it is"
        );
        assert_eq!(
            catalogue.resin(&id).expect("there").profile.name,
            "Grey faster"
        );
    }

    /// P-11: a copy made by an older release stands over the shipped profile and nothing
    /// in it says so, so the form is told what the build ships under the same id.
    #[test]
    fn a_copy_that_differs_from_the_shipped_machine_carries_it() {
        let mut catalogue = writable("own-copy");
        let shipped = catalogue
            .shipped_printer(SATURN)
            .expect("a shipped machine");
        let edited = PrinterProfile {
            name: "Saturn, as I measured it".to_owned(),
            ..shipped.clone()
        };
        catalogue.save_printer(SATURN, &edited).expect("writable");

        let mut settings = Settings::default();
        settings.pick(&catalogue, Node::Printer(SATURN.to_owned()));
        let draft = settings.printer.as_ref().expect("a printer is open");
        assert_eq!(draft.shipped(), Some(&shipped));

        catalogue.save_printer(SATURN, &shipped).expect("writable");
        settings.pick(&catalogue, Node::Printer(SATURN.to_owned()));
        let draft = settings.printer.as_ref().expect("a printer is open");
        assert_eq!(
            draft.shipped(),
            None,
            "a copy that matches what is shipped has nothing to say"
        );
    }

    #[test]
    fn a_machine_nobody_shipped_has_no_shipped_profile_behind_it() {
        let mut catalogue = writable("own-machine");
        let id = new_printer(&mut catalogue).expect("writable");
        let mut settings = Settings::default();
        settings.pick(&catalogue, Node::Printer(id));
        assert!(settings.printer.as_ref().expect("open").shipped().is_none());
    }

    #[test]
    fn only_a_profile_nobody_shipped_can_be_deleted() {
        let mut catalogue = writable("delete");
        assert!(!can_delete(&catalogue, Kind::Printer, MARS));
        let id = new_printer(&mut catalogue).expect("writable");
        assert!(can_delete(&catalogue, Kind::Printer, &id));
    }

    #[test]
    fn the_support_page_opens_on_the_profile_a_group_was_taken_from() {
        let catalogue = writable("supports");
        let mut settings = Settings::default();
        settings.open_supports(&catalogue, &SupportProfile::heavy());
        assert_eq!(settings.section, Section::Supports);
        assert_eq!(settings.support.as_ref().expect("open").id, "heavy");
    }
}
