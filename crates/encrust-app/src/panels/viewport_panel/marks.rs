//! What the Repair and Check tools mark over the models, and the camera's flight to it or
//! to where models meet.

use core_geometry::Scalar;

use super::repair;
use crate::camera::CameraTurn;
use crate::panels::{Window, section};
use crate::workspace::Tool;

/// The seam where models meet, in every tool, then what the Repair tool found or the issues
/// Check went to while the plate and the cut at `section_mm` stay as they were, in hand.
pub(super) fn marked(
    window: &Window,
    section_mm: Option<Scalar>,
    (rect, cursor): (egui::Rect, Option<egui::Pos2>),
) -> crate::highlight::Highlight {
    let mut marked = window.doc.meeting.highlight(&window.doc.scene);
    let tool = by_tool(window, section_mm, (rect, cursor));
    marked.faces.extend(tool.faces);
    marked.edges.extend(tool.edges);
    marked
}

fn by_tool(
    window: &Window,
    section_mm: Option<Scalar>,
    (rect, cursor): (egui::Rect, Option<egui::Pos2>),
) -> crate::highlight::Highlight {
    match *window.tool {
        Tool::Repair => repair::marked(window, rect, cursor),
        Tool::Check => window.tools.check.highlight(crate::check::Stage {
            plate: crate::preview::plate_fingerprint(&window.doc.scene),
            cut_mm: section_mm,
        }),
        _ => crate::highlight::Highlight::default(),
    }
}

/// Flies the camera to where models meet, from a press on a tag, or where the Repair tool's
/// last press or Check's last pick went, on the frame that place is worked out, seen past
/// what the section leaves drawn.
pub(super) fn fly_to_marked(ui: &egui::Ui, window: &mut Window, rect: egui::Rect) {
    let by_tool = match *window.tool {
        Tool::Repair => repair::flight(window),
        Tool::Check => window.tools.check.take_flight(),
        _ => None,
    };
    let spot = window.doc.meeting.take_flight().or(by_tool);
    let Some(spot) = spot else {
        return;
    };
    let shown = section::shown(window);
    let scene = &window.doc.scene;
    let aspect = rect.width() / rect.height().max(1.0);
    let hidden = |eye, point| crate::pick::sight_blocked(scene, eye, point, shown);
    let to = window.view.camera.focusing(&spot, aspect, hidden);
    let now = ui.input(|input| input.time);
    let mut flight = CameraTurn::flight(window.view.camera, to, now);
    if crate::ui::theme::motion::reduced(ui.ctx()) {
        flight.length_s = 0.0;
    }
    window.view.turn = Some(flight);
    ui.ctx().request_repaint();
}
