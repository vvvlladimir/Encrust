use super::playback::{fraction, height_range};
use super::{floor_height, layer_at};
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{ReadoutWords, layer_readout};
use crate::workspace::Mode;
use core_geometry::Scalar;

/// Where the cut stands on the strip, and what it counts in: layers of the stack in the
/// Preview mode, layer heights of the model in the Prepare mode.
pub(super) struct Reading {
    /// Share of the way up, from zero at the plate to one at the top, or `None` with
    /// nothing to scrub.
    pub(super) at: Option<f32>,
    /// Share of the way up the floor stands and the first layer it leaves, counted from
    /// one, or `None` while nothing is cut away from below.
    pub(super) floor: Option<(f32, usize)>,
    pub(super) layer: usize,
    pub(super) layers: usize,
    pub(super) height_mm: Scalar,
    pub(super) top_mm: Scalar,
}

impl Reading {
    pub(super) fn of(window: &Window) -> Self {
        match *window.mode {
            Mode::Preview => {
                let preview = &window.machine.preview;
                let count = preview.layer_count();
                let last = count.saturating_sub(1);
                Self {
                    at: (count > 0)
                        .then(|| fraction(0.0, last as Scalar, preview.layer() as Scalar)),
                    floor: floor_height(Mode::Preview, &window.view.section, preview).map(
                        |floor| {
                            let first =
                                (preview.layer_topping_near(floor) + 1).min(preview.layer());
                            (fraction(0.0, last as Scalar, first as Scalar), first + 1)
                        },
                    ),
                    layer: if count > 0 { preview.layer() + 1 } else { 0 },
                    layers: count,
                    height_mm: preview.layer_z().unwrap_or_default(),
                    top_mm: preview.stack_top_mm().unwrap_or_default(),
                }
            }
            Mode::Prepare => {
                let Some((bottom, top)) = height_range(window.doc) else {
                    return Self::nothing();
                };
                let height = window.view.section.height_mm.unwrap_or(top);
                let (layer, layers) = layer_at(
                    height,
                    (bottom, top),
                    window.machine.slicing.layer_height_mm(),
                );
                let floor =
                    floor_height(Mode::Prepare, &window.view.section, &window.machine.preview).map(
                        |floor| {
                            let (under, _) = layer_at(
                                floor,
                                (bottom, top),
                                window.machine.slicing.layer_height_mm(),
                            );
                            (fraction(bottom, top, floor), under + 1)
                        },
                    );
                Self {
                    at: Some(fraction(bottom, top, height)),
                    floor,
                    layer,
                    layers,
                    height_mm: height,
                    top_mm: top,
                }
            }
        }
    }

    pub(super) fn nothing() -> Self {
        Self {
            at: None,
            floor: None,
            layer: 0,
            layers: 0,
            height_mm: 0.0,
            top_mm: 0.0,
        }
    }

    /// The readout of the layer the cut stands at, and its height. Answers with the layer
    /// typed, on Enter.
    pub(super) fn show(&self, ui: &mut egui::Ui) -> Option<usize> {
        let words = ReadoutWords {
            of: tr!("section-of", count = self.layers),
            height: mm(self.height_mm),
            hover: tr!(
                "section-height-of",
                height = format!("{:.2}", self.height_mm),
                top = format!("{:.2}", self.top_mm)
            ),
            type_hint: tr!("section-type-layer"),
            field_name: tr!("section-layer-field", layer = self.layer),
            none: tr!("section-no-layers"),
            none_hover: tr!("section-no-layers-yet"),
        };
        let layer = self.at.map(|_| self.layer);
        layer_readout(ui, layer, &words)
    }
}

/// A height in the strip's narrow column: two decimals, one from a hundred millimetres up.
pub(super) fn mm(height_mm: Scalar) -> String {
    let value = if height_mm < 100.0 {
        format!("{height_mm:.2}")
    } else {
        format!("{height_mm:.1}")
    };
    tr!("value-mm", value = value)
}
