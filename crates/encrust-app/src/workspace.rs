use core_geometry::Scalar;
use serde::{Deserialize, Serialize};

use crate::ui::{icon, theme};

/// What the window is doing with the plate: laying it out, or looking at the layers it
/// will print. One is the editing half of the application, the other the reading half.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Prepare,
    Preview,
}

impl Mode {
    pub const ALL: [Self; 2] = [Self::Prepare, Self::Preview];

    pub fn label(self) -> &'static str {
        match self {
            Self::Prepare => "Prepare",
            Self::Preview => "Preview",
        }
    }
}

/// What a click in the viewport does, and which section of the inspector it opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Supports,
    Hollow,
    Drain,
    Cut,
    Relief,
    Layers,
    Measure,
}

impl Tool {
    /// The groups of the rail, in the order it draws them, with a rule between each.
    pub const PLACING: [Self; 1] = [Self::Select];
    pub const SHAPING: [Self; 5] = [
        Self::Supports,
        Self::Hollow,
        Self::Drain,
        Self::Cut,
        Self::Relief,
    ];
    /// What the stack is cut with, and what only reads the plate.
    pub const PRINTING: [Self; 2] = [Self::Layers, Self::Measure];

    /// Every variant, which is what a test walks to prove none was left unplaced.
    #[cfg(test)]
    pub const ALL: [Self; 8] = [
        Self::Select,
        Self::Supports,
        Self::Hollow,
        Self::Drain,
        Self::Cut,
        Self::Relief,
        Self::Layers,
        Self::Measure,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Supports => "Supports",
            Self::Hollow => "Hollow",
            Self::Drain => "Drain holes",
            Self::Cut => "Cut and split",
            Self::Relief => "Relief",
            Self::Layers => "Layers and exposure",
            Self::Measure => "Measure",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Select => icon::SELECT,
            Self::Supports => icon::SUPPORTS,
            Self::Hollow => icon::HOLLOW,
            Self::Drain => icon::DRAIN,
            Self::Cut => icon::CUT,
            Self::Relief => icon::RELIEF,
            Self::Layers => icon::SLICE,
            Self::Measure => icon::MEASURE,
        }
    }
}

/// Where the viewport cuts the plate's contents, so that the slider down the right of the
/// stage reads as a section through the part rather than as a layer counter alone.
///
/// Only the Prepare mode keeps a height here. The Preview mode cuts at the layer it is
/// showing, which the preview already owns; see `docs/decisions/0061`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Section {
    /// Height above the plate, millimetres, or `None` while the whole model is drawn.
    pub height_mm: Option<Scalar>,
}

/// The grid of copies the Array button lays out.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Array {
    pub columns: u32,
    pub rows: u32,
    /// Space left between one copy and the next, millimetres.
    pub gap_mm: Scalar,
}

impl Default for Array {
    fn default() -> Self {
        Self {
            columns: 2,
            rows: 2,
            gap_mm: 5.0,
        }
    }
}

/// What the window shows besides the models.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewOptions {
    pub grid: bool,
    /// Whether the plate panel is unfolded down the left of the stage.
    pub plate_panel: bool,
    /// Whether the sheet of keys is up over the window.
    pub sheet: bool,
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
            plate_panel: true,
            sheet: false,
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

    /// A variant added and not placed would silently never be reachable.
    #[test]
    fn every_tool_is_on_the_rail() {
        for tool in Tool::ALL {
            let on_rail = Tool::PLACING.contains(&tool)
                || Tool::SHAPING.contains(&tool)
                || Tool::PRINTING.contains(&tool);
            assert!(on_rail, "{} is on no rail group", tool.label());
        }
    }
}
