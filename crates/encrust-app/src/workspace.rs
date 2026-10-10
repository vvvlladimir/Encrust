pub use core_engine::project::Array;
use core_geometry::Scalar;

use crate::ui::{icon, theme};

/// What the window is doing with the plate: laying it out, or looking at the layers it
/// will print. The layer strip's views switch it: the model alone is Prepare, the model
/// beside its layer and the layer alone are Preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Prepare,
    Preview,
}

/// What a click in the viewport does, and what the inspector beside the rail shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Position,
    Measure,
    Hollow,
    Drain,
    Cut,
    Relief,
    /// Growing supports automatically, and picking the parts of those already standing.
    Supports,
    /// Placing supports by hand, painting where they go and where they must not.
    Paint,
    /// The shape of one support, part by part, in the profile of the group in hand.
    Shape,
    Check,
    Export,
    /// A form rather than a tool: opened from the top bar, never from the rail.
    PrintSettings,
}

impl Tool {
    /// The groups of the rail, in the order it draws them, with a rule between each; see
    /// `docs/decisions/0218`.
    pub const RAIL: [&'static [Self]; 4] = [
        &[Self::Select, Self::Position, Self::Measure],
        &[Self::Hollow, Self::Drain, Self::Cut, Self::Relief],
        &[Self::Supports, Self::Paint, Self::Shape],
        &[Self::Check, Self::Export],
    ];

    /// Every variant, which is what a test walks to prove none was left unplaced.
    #[cfg(test)]
    pub const ALL: [Self; 13] = [
        Self::Select,
        Self::Position,
        Self::Measure,
        Self::Hollow,
        Self::Drain,
        Self::Cut,
        Self::Relief,
        Self::Supports,
        Self::Paint,
        Self::Shape,
        Self::Check,
        Self::Export,
        Self::PrintSettings,
    ];

    /// The tools of the rail, top to bottom.
    #[cfg(test)]
    pub fn on_rail() -> impl Iterator<Item = Self> {
        Self::RAIL.into_iter().flatten().copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Position => "Position",
            Self::Measure => "Measure",
            Self::Hollow => "Hollow",
            Self::Drain => "Drain",
            Self::Cut => "Cut",
            Self::Relief => "Relief",
            Self::Supports => "Supports",
            Self::Paint => "Paint",
            Self::Shape => "Shape",
            Self::Check => "Check",
            Self::Export => "Export",
            Self::PrintSettings => "Print settings",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Select => icon::SELECT,
            Self::Position => icon::POSITION,
            Self::Measure => icon::MEASURE,
            Self::Hollow => icon::HOLLOW,
            Self::Drain => icon::DRAIN,
            Self::Cut => icon::CUT,
            Self::Relief => icon::RELIEF,
            Self::Supports => icon::SUPPORTS,
            Self::Paint => icon::PAINT,
            Self::Shape => icon::SHAPE,
            Self::Check => icon::CHECK,
            Self::Export => icon::EXPORT,
            Self::PrintSettings => icon::PARAMETERS,
        }
    }

    /// Whether the plate is shaded by what needs holding up while this tool is in hand.
    pub fn shows_overhangs(self) -> bool {
        matches!(self, Self::Supports | Self::Paint)
    }
}

/// Where the viewport cuts the plate's contents, so that the layer strip under the stage
/// reads as a section through the part rather than as a layer counter alone.
///
/// Only the Prepare mode keeps a height here. The Preview mode cuts at the layer it is
/// showing, which the preview already owns; see `docs/decisions/0061`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Section {
    /// Height above the plate, millimetres, or `None` while the whole model is drawn.
    pub height_mm: Option<Scalar>,
    /// Whether the cut is running up the model on its own, which is what Play does in
    /// this mode.
    pub playing: bool,
}

/// What the window shows besides the models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewOptions {
    pub grid: bool,
    /// Whether the models are drawn seen through, so a cavity and what stands in it can be
    /// looked into. The view tools and the View menu are the only things that turn it on;
    /// see ADR 0190, 0198.
    pub xray: bool,
    /// Whether the plate panel is unfolded down the left of the stage.
    pub plate_panel: bool,
    /// Whether the sheet of keys is up over the window.
    pub sheet: bool,
    /// Whether Preview gives the whole stage to the layer mask rather than half of it.
    pub mask_only: bool,
    /// Whether Preview's column shows the issues found in the stack instead of the layer.
    pub issues: bool,
    /// Width of the plate panel and of the inspector, points. Both are the user's: a
    /// column that sized itself to its contents would jump whenever a tool changed.
    pub plate_w: f32,
    pub inspector_w: f32,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            grid: true,
            xray: false,
            plate_panel: true,
            sheet: false,
            mask_only: false,
            issues: false,
            plate_w: theme::SCENE_W,
            inspector_w: theme::INSPECTOR_W,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_its_own_glyph_and_name() {
        for (index, tool) in Tool::ALL.iter().enumerate() {
            for other in &Tool::ALL[index + 1..] {
                assert_ne!(tool.glyph(), other.glyph());
                assert_ne!(tool.label(), other.label());
            }
        }
    }

    /// A variant added and not placed would silently never be reachable; the print
    /// settings alone are opened from the top bar instead.
    #[test]
    fn every_tool_but_the_print_settings_is_on_the_rail() {
        for tool in Tool::ALL {
            let on_rail = Tool::on_rail().any(|placed| placed == tool);
            assert_eq!(
                on_rail,
                tool != Tool::PrintSettings,
                "{} is in the wrong place",
                tool.label()
            );
        }
    }
}
