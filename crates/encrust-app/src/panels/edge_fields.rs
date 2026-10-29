//! How a mask's edge is shaded, as one of four looks or the numbers behind them, in both
//! the print settings and the resin form.

use std::num::NonZeroU8;

use printer_profiles::EdgeSmoothing;

use crate::i18n::tr;
use crate::ui::{
    Segment, Segmented, count_row, describe, hint, kept, list, list_row, nested, switch,
};

/// Eight greys is anti-aliasing level 8, and the most a panel
/// holds; see `docs/decisions/0113`.
const DEFAULT_GREY_LEVELS: NonZeroU8 = NonZeroU8::new(8).expect("eight is not zero");

/// The greys a sharp edge is rounded to: few enough that small lettering keeps its corners.
const SHARP_GREY_LEVELS: u8 = 4;
/// The blur a soft edge is faded over, pixels: past two it dims the wall inside the edge.
const SOFT_BLUR_PX: u8 = 2;

/// A Look | Number switch, then the four looks or the switch and numbers behind them.
pub fn rows(ui: &mut egui::Ui, edges: &mut EdgeSmoothing) {
    let look = Look::of(edges);
    let by_look_id = ui.id().with("edges-by-look");
    let mut by_look = kept::get(ui.ctx(), by_look_id).unwrap_or(look.is_some());
    let choices = [
        Segment::new(true, tr!("layers-look")),
        Segment::new(false, tr!("layers-number")),
    ];
    Segmented::new(&choices)
        .width(ui.available_width())
        .show(ui, &mut by_look);
    kept::set(ui.ctx(), by_look_id, &by_look);

    if by_look {
        if let Some(picked) = looks(ui, look) {
            picked.apply(edges);
        }
        return;
    }
    switch(ui, &mut edges.anti_alias, &tr!("layers-anti-aliased"));
    if edges.anti_alias {
        nested(ui, |ui| {
            grey_levels(ui, &mut edges.grey_levels);
            blur_row(ui, &mut edges.blur_px);
        });
    }
}

/// The greys and blur an edge takes, in a few words for a heading.
pub fn summary(edges: &EdgeSmoothing) -> String {
    let blur = edges.blur_px;
    match (edges.anti_alias, edges.grey_levels, blur) {
        (false, _, _) => tr!("look-off"),
        (true, Some(levels), 0) => tr!("layers-levels", count = levels.get()),
        (true, Some(levels), _) => tr!("layers-levels-blur", count = levels.get(), blur = blur),
        (true, None, 0) => tr!("layers-all-greys"),
        (true, None, _) => tr!("layers-all-greys-blur", blur = blur),
    }
}

/// A named way for an edge to come out, standing for the numbers that make it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    Off,
    Sharp,
    Balanced,
    Soft,
}

impl Look {
    const ALL: [Self; 4] = [Self::Off, Self::Sharp, Self::Balanced, Self::Soft];

    fn name(self) -> String {
        match self {
            Self::Off => tr!("look-off"),
            Self::Sharp => tr!("look-sharp"),
            Self::Balanced => tr!("look-balanced"),
            Self::Soft => tr!("look-soft"),
        }
    }

    fn line(self) -> String {
        match self {
            Self::Off => tr!("look-off-line"),
            Self::Sharp => tr!("look-sharp-line"),
            Self::Balanced => tr!("look-balanced-line"),
            Self::Soft => tr!("look-soft-line"),
        }
    }

    /// The greys an edge is rounded to and its blur in pixels, or `None` for no smoothing,
    /// which leaves the numbers as they were for the next time it is turned on.
    fn settings(self) -> Option<(Option<NonZeroU8>, u8)> {
        match self {
            Self::Off => None,
            Self::Sharp => Some((NonZeroU8::new(SHARP_GREY_LEVELS), 0)),
            Self::Balanced => Some((None, 0)),
            Self::Soft => Some((None, SOFT_BLUR_PX)),
        }
    }

    fn apply(self, edges: &mut EdgeSmoothing) {
        edges.anti_alias = self.settings().is_some();
        if let Some((grey_levels, blur_px)) = self.settings() {
            (edges.grey_levels, edges.blur_px) = (grey_levels, blur_px);
        }
    }

    /// The look these numbers make, or `None` for numbers typed by hand.
    fn of(edges: &EdgeSmoothing) -> Option<Self> {
        if !edges.anti_alias {
            return Some(Self::Off);
        }
        Self::ALL
            .into_iter()
            .find(|look| look.settings() == Some((edges.grey_levels, edges.blur_px)))
    }
}

/// The four looks as a column of choices, the one in use lit. Answers the one pressed.
fn looks(ui: &mut egui::Ui, current: Option<Look>) -> Option<Look> {
    let picked = list(ui, |ui| {
        Look::ALL.into_iter().fold(None, |picked, look| {
            let on = current == Some(look);
            let line = look.line();
            let row = list_row(ui, &look.name(), None, on, None, None, Some(&line), None);
            if row.picked { Some(look) } else { picked }
        })
    });
    if current.is_none() {
        hint(ui, &tr!("layers-by-hand"));
    }
    picked
}

/// How many greys an edge is rounded to. Off is every grey the panel will take.
fn grey_levels(ui: &mut egui::Ui, levels: &mut Option<NonZeroU8>) {
    let mut limited = levels.is_some();
    if switch(ui, &mut limited, &tr!("layers-round-grey")).changed() {
        *levels = limited.then_some(DEFAULT_GREY_LEVELS);
    }
    if let Some(count) = levels.as_mut() {
        nested(ui, |ui| {
            let mut value = u32::from(count.get());
            if count_row(ui, &tr!("layers-grey-levels"), &mut value, "", 1.0, 2..=16) {
                *count = NonZeroU8::new(value as u8).unwrap_or(DEFAULT_GREY_LEVELS);
            }
        });
    }
}

/// How many pixels either side an edge is faded over. Zero keeps it sharp.
fn blur_row(ui: &mut egui::Ui, blur_px: &mut u8) {
    let mut value = u32::from(*blur_px);
    if count_row(
        ui,
        &tr!("layers-edge-blur"),
        &mut value,
        &tr!("unit-px"),
        1.0,
        0..=8,
    ) {
        *blur_px = value as u8;
    }
    if *blur_px > 2 {
        describe(ui, &tr!("layers-blur-note"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_is_read_back_from_the_numbers_it_sets() {
        let mut edges = EdgeSmoothing::default();
        for look in Look::ALL {
            look.apply(&mut edges);
            assert_eq!(
                Look::of(&edges),
                Some(look),
                "{} did not survive",
                look.name()
            );
        }
    }

    #[test]
    fn turning_smoothing_off_keeps_the_numbers_for_later() {
        let mut edges = EdgeSmoothing::default();
        Look::Soft.apply(&mut edges);
        Look::Off.apply(&mut edges);
        assert!(!edges.anti_alias);
        assert_eq!(
            edges.blur_px, SOFT_BLUR_PX,
            "the soft blur waits to be turned on again"
        );
    }

    #[test]
    fn a_new_plate_starts_on_the_balanced_look() {
        let slicing = crate::slicing::Slicing::default();
        assert_eq!(Look::of(&slicing.edges), Some(Look::Balanced));
    }
}
