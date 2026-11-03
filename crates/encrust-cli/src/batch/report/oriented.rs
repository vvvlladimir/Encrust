//! How `--orient` stood a model, and the other ways the search offered with what each printed
//! (ADR 0403).

use core_geometry::Scalar;
use serde::Serialize;

use crate::pipeline::{OrientChoice, OrientSummary, rotate_degrees};

#[derive(Debug, Serialize)]
pub struct Oriented {
    pub degrees: Scalar,
    /// The angles `--rotate X,Y,Z` repeats the turn with, degrees.
    pub rotate_deg: [Scalar; 3],
    pub peak_section_mm2: Scalar,
    pub overhang_mm2: Scalar,
    /// The search's best ways, as many as `--orient-choices` asked for, best first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
}

/// One way to stand the model, numbered as `--orient-pick` takes it.
#[derive(Debug, Serialize)]
pub struct Choice {
    pub pick: usize,
    pub rotate_deg: [Scalar; 3],
    pub on_plate: bool,
    /// Whether it trades well enough against the first to be worth offering beside it.
    pub offered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub printed: Option<Printed>,
}

/// What a choice printed on a coarse trial stack: for comparing choices, not a final check.
#[derive(Debug, Serialize)]
pub struct Printed {
    pub contacts: usize,
    /// Contacts on faces that look up in the model's own file: marks on a visible surface.
    pub top_contacts: usize,
    pub kept_clear_touched: usize,
    pub hanging: usize,
    pub supports_ml: f32,
    pub resin_ml: f32,
    pub layers: usize,
    pub stack_mm: f32,
    pub pull_n: f64,
    pub islands: usize,
    pub suction_cups: usize,
    pub peels: usize,
    pub overflows: bool,
}

impl Oriented {
    pub fn of(found: &OrientSummary) -> Self {
        Self {
            degrees: found.degrees(),
            rotate_deg: found.rotate_degrees().to_array(),
            peak_section_mm2: found.peak_mm2,
            overhang_mm2: found.overhang_mm2,
            choices: found
                .choices
                .iter()
                .enumerate()
                .map(|(at, choice)| Choice::of(at + 1, choice))
                .collect(),
        }
    }
}

impl Choice {
    fn of(pick: usize, choice: &OrientChoice) -> Self {
        Self {
            pick,
            rotate_deg: rotate_degrees(choice.rotation).to_array(),
            on_plate: choice.on_plate,
            offered: choice.offered,
            printed: choice.printout.map(|out| Printed {
                contacts: out.contacts,
                top_contacts: out.top_contacts,
                kept_clear_touched: out.kept_clear_touched,
                hanging: out.hanging,
                supports_ml: out.supports_ml,
                resin_ml: out.resin_ml,
                layers: out.layers,
                stack_mm: out.stack_mm,
                pull_n: out.pull_n,
                islands: out.islands,
                suction_cups: out.cups,
                peels: out.peels,
                overflows: out.overflows,
            }),
        }
    }
}
