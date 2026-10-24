//! The Settings window's state: which page is open, what is picked on it, and the profile
//! being edited, which is written back as it changes. See `docs/design/profiles.md`.

pub use printer_profiles::unique_id;
use printer_profiles::{Catalogue, Kind, PrinterProfile, SupportProfile};

use crate::i18n::tr;
use crate::undo::Frame;

mod drafts;
mod library;
pub use drafts::{Draft, ResinDraft};
pub use library::{
    add_resin, can_delete, delete_resin, duplicate_resin, installed, installed_printers,
    is_thrown_away_with_the_printer, keep_support, new_printer, new_resin, new_support,
    other_printers, pool_for, rename_resin, resins_of, take_resin_off,
};

/// A page of the Settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Supports,
    /// The Hollow tool's named settings.
    Hollow,
    /// The Drain tool's named settings.
    Drain,
    /// How the window itself behaves: where it starts, how it zooms, where files go.
    General,
    /// Whether to look for a new release, and the one found.
    Updates,
}

impl Section {
    /// The pages that list profiles, over the ones that hold the window's own settings.
    pub const PROFILES: [Self; 3] = [Self::Supports, Self::Hollow, Self::Drain];

    #[cfg(not(target_arch = "wasm32"))]
    pub const OWN: [Self; 2] = [Self::General, Self::Updates];

    /// A page loads the latest release every time it is opened, so it has nothing to update.
    #[cfg(target_arch = "wasm32")]
    pub const OWN: [Self; 1] = [Self::General];

    pub fn label(self) -> String {
        match self {
            Self::Supports => tr!("settings-page-supports"),
            Self::Hollow => tr!("settings-page-hollow"),
            Self::Drain => tr!("settings-page-drain"),
            Self::General => tr!("settings-page-general"),
            Self::Updates => tr!("settings-page-updates"),
        }
    }

    /// The kind of profile the page lists, when it lists any.
    pub fn kind(self) -> Option<Kind> {
        match self {
            Self::Supports => Some(Kind::Support),
            Self::Hollow => Some(Kind::Hollow),
            Self::Drain => Some(Kind::Drain),
            Self::General | Self::Updates => None,
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

    pub fn label(self) -> String {
        match self {
            Self::Resins => tr!("settings-tab-resins"),
            Self::Machine => tr!("settings-tab-machine"),
            Self::Network => tr!("settings-tab-network"),
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
    /// A support profile or a tool's preset of the user's own, from the Settings window.
    Profile(Kind, String),
    /// Every copy of a shipped profile that would not read, the shipped one in use instead.
    Unreadable,
}

/// What the machine library has open: a brand, and what its models are filtered by.
#[derive(Default)]
pub struct Library {
    pub brand: Option<String>,
    pub search: String,
}

/// Which profile's form is in view, and so which one undo reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Printer,
    Resin,
    Support,
}

/// What the Settings window and the Machine and resin window are showing.
#[derive(Default)]
pub struct Settings {
    /// Whether the Settings window is over the plate.
    pub open: bool,
    pub section: Section,
    /// Whether the support profile picked in the table is open in its form instead.
    pub editing_support: bool,
    /// The preset picked in the Hollow or Drain table, by id.
    pub preset: Option<String>,
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

    /// Opens the window on the form of the support profile `profile` was taken from.
    pub fn open_supports(&mut self, catalogue: &Catalogue, profile: &SupportProfile) {
        self.open = true;
        self.section = Section::Supports;
        self.editing_support = true;
        let entry = catalogue
            .supports()
            .find(|entry| entry.profile == *profile)
            .or_else(|| catalogue.supports().next());
        if let Some(entry) = entry {
            self.support = Some(Draft::new(entry.id.clone(), entry.profile.clone()));
        }
    }

    /// Turns to another page, with nothing on it picked or open.
    pub fn turn_to(&mut self, section: Section) {
        self.section = section;
        self.editing_support = false;
        self.preset = None;
    }

    /// The form in view in whichever of the two windows is open, if one is.
    fn form_in_view(&self) -> Option<Form> {
        if self.machines {
            return match self.tab {
                _ if self.library.is_some() => None,
                Tab::Resins => self.editing_resin.then_some(Form::Resin),
                Tab::Machine | Tab::Network => Some(Form::Printer),
            };
        }
        (self.open && self.section == Section::Supports && self.editing_support)
            .then_some(Form::Support)
    }

    /// Records the edit of the form in view once its gesture is over.
    pub fn observe(&mut self, frame: Frame) {
        match self.form_in_view() {
            Some(Form::Printer) => self.printer.as_mut().map(|draft| draft.observe(frame)),
            Some(Form::Resin) => self.resin.as_mut().map(|resin| resin.draft.observe(frame)),
            Some(Form::Support) => self.support.as_mut().map(|draft| draft.observe(frame)),
            None => None,
        };
    }

    /// Takes back the last edit of the form in view.
    pub fn undo(&mut self) -> bool {
        match self.form_in_view() {
            Some(Form::Printer) => self.printer.as_mut().is_some_and(Draft::undo),
            Some(Form::Resin) => self.resin.as_mut().is_some_and(ResinDraft::undo),
            Some(Form::Support) => self.support.as_mut().is_some_and(Draft::undo),
            None => false,
        }
    }

    /// Puts back the last edit of the form in view that undo took away.
    pub fn redo(&mut self) -> bool {
        match self.form_in_view() {
            Some(Form::Printer) => self.printer.as_mut().is_some_and(Draft::redo),
            Some(Form::Resin) => self.resin.as_mut().is_some_and(ResinDraft::redo),
            Some(Form::Support) => self.support.as_mut().is_some_and(Draft::redo),
            None => false,
        }
    }

    /// Closes the Settings window, and the question over it.
    pub fn close(&mut self) {
        self.open = false;
        self.editing_support = false;
        self.confirm = None;
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

#[cfg(test)]
mod tests;
