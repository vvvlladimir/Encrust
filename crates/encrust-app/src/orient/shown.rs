//! Keeping a clean surface: the switch the window remembers, and the brush that paints
//! where a model shows (ADR 0410).

use core_geometry::Vec2;
use serde::{Deserialize, Serialize};

use super::OrientTool;

/// What Auto-orient keeps between runs beside its preset: whether it holds to a clean
/// surface, how wide the brush that paints what shows is, and how far a model may tilt.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OrientState {
    pub clean_surface: bool,
    pub brush_radius_mm: f32,
    /// Degrees from the way the model stands, or `None` for any way at all.
    pub max_tilt_deg: Option<f32>,
}

impl Default for OrientState {
    fn default() -> Self {
        Self {
            clean_surface: false,
            brush_radius_mm: 2.0,
            max_tilt_deg: None,
        }
    }
}

/// The brush that paints the faces of a model that show, which a clean surface keeps
/// supports off, and the stroke it has going.
#[derive(Debug, Clone, Copy)]
pub struct ShownBrush {
    pub brushing: bool,
    pub radius_mm: f32,
    pub stroke: bool,
    pub last_daub: Option<Vec2>,
}

impl Default for ShownBrush {
    fn default() -> Self {
        Self {
            brushing: false,
            radius_mm: OrientState::default().brush_radius_mm,
            stroke: false,
            last_daub: None,
        }
    }
}

impl OrientTool {
    /// Whether a drag over a model paints where it shows.
    pub fn paints_shown(&self) -> bool {
        self.settings.clean_surface && self.shown_brush.brushing && !self.is_running()
    }

    /// What the window remembers of the tool.
    pub fn state(&self) -> OrientState {
        OrientState {
            clean_surface: self.settings.clean_surface,
            brush_radius_mm: self.shown_brush.radius_mm,
            max_tilt_deg: self.settings.max_tilt_deg,
        }
    }

    /// Stands the tool under `state`; a brush in hand is put down where the switch goes off.
    pub fn set_state(&mut self, state: OrientState) {
        self.settings.clean_surface = state.clean_surface;
        self.shown_brush.radius_mm = state.brush_radius_mm;
        self.settings.max_tilt_deg = state.max_tilt_deg;
        self.shown_brush.brushing &= state.clean_surface;
    }
}
