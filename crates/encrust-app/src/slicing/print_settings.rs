//! What the Print settings panel edits. It lasts while the plate is in hand: a project opened,
//! a new one, a plate added and the next run take the printer's and resin's again (ADR 0380).

use std::num::NonZeroU8;

use core_format::ExposureRange;
use core_slicer::{AdaptiveSettings, ONE_SAMPLE};
use printer_profiles::{EdgeSmoothing, MaterialProfile};

use super::{Rescaled, Slicing};

/// The print settings as they stand, which only the undo stack keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintSettings {
    /// The printer and resin ids they were set on; another pair is not given them back.
    machine: (Option<String>, Option<String>),
    material: MaterialProfile,
    adaptive: Option<AdaptiveSettings>,
    exposure: Vec<ExposureRange>,
    samples: NonZeroU8,
    edges: EdgeSmoothing,
    rescaled: Option<Rescaled>,
}

impl Slicing {
    /// What the Print settings panel is set to now.
    pub fn print_settings(&self) -> PrintSettings {
        PrintSettings {
            machine: self.machine_ids(),
            material: self.material.clone(),
            adaptive: self.adaptive,
            exposure: self.exposure.clone(),
            samples: self.samples,
            edges: self.edges,
            rescaled: self.rescaled.clone(),
        }
    }

    /// Puts `settings` back, when they were set on the printer and resin in hand.
    pub fn put_print_settings(&mut self, settings: PrintSettings) {
        if settings.machine != self.machine_ids() {
            return;
        }
        self.material = settings.material;
        self.adaptive = settings.adaptive;
        self.exposure = settings.exposure;
        self.samples = settings.samples;
        self.edges = settings.edges;
        self.rescaled = settings.rescaled;
    }

    /// The printer's and resin's print settings in place of what the panel was set to, as
    /// a fresh start rather than an edit to take back.
    pub fn take_profile_settings(&mut self) {
        self.put_print_settings(self.profile_settings());
        self.chosen += 1;
    }

    /// Whether the panel holds anything other than the printer's and resin's settings.
    pub fn print_settings_edited(&self) -> bool {
        self.print_settings() != self.profile_settings()
    }

    /// What the printer and resin in hand set the panel to.
    pub fn profile_settings(&self) -> PrintSettings {
        PrintSettings {
            machine: self.machine_ids(),
            material: self.starting_material(),
            edges: self.base_material.edges,
            adaptive: None,
            exposure: Vec::new(),
            samples: ONE_SAMPLE,
            rescaled: None,
        }
    }

    fn machine_ids(&self) -> (Option<String>, Option<String>) {
        (self.printer_id.clone(), self.resin_id.clone())
    }
}
