//! The Faces mode: faces picked with a brush to turn or drop, open edges picked to bridge.

use crate::i18n::tr;
use crate::panels::Window;
use crate::repair::{HandEdit, Picking};
use crate::scene::ObjectId;
use crate::ui::{
    Segment, Segmented, blocked_hover, busy, companion_button, describe, icon, number_row,
    primary_button, quiet_button, readings, section, theme, tone,
};

/// Per point of drag: brush in tenths of a millimetre, the angle in degrees.
const BRUSH_STEP: f64 = 0.05;
const ANGLE_STEP: f64 = 0.5;

/// What a click picks, the brush it picks with, and what is picked.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    if window.tools.repair.picking == Picking::Edges {
        let ids: Vec<ObjectId> = window.doc.scene.targets().map(|object| object.id).collect();
        window.doc.repairs.found.place(&window.doc.scene, &ids);
    }
    let tool = &mut window.tools.repair;
    section(ui, &tr!("repair-pick"), None, |ui| {
        describe(
            ui,
            &match tool.picking {
                Picking::Faces => tr!("repair-pick-faces-note"),
                Picking::Edges => tr!("repair-pick-edges-note"),
            },
        );
        let segments = [
            Segment::new(Picking::Faces, tr!("repair-pick-faces")),
            Segment::new(Picking::Edges, tr!("repair-pick-edges")),
        ];
        Segmented::new(&segments)
            .width(ui.available_width())
            .show(ui, &mut tool.picking);
    });
    if tool.picking == Picking::Faces {
        section(ui, &tr!("repair-brush"), None, |ui| {
            let state = &mut tool.state;
            let mm = tr!("unit-mm");
            let radius = &mut state.brush_radius_mm;
            number_row(
                ui,
                &tr!("repair-brush-radius"),
                radius,
                &mm,
                BRUSH_STEP,
                0.1..=20.0,
                2,
            );
            let angle = &mut state.flood_angle_deg;
            let degrees = tr!("unit-degrees");
            number_row(
                ui,
                &tr!("repair-flood-angle"),
                angle,
                &degrees,
                ANGLE_STEP,
                0.0..=90.0,
                1,
            );
        });
    }
    picked(ui, window);
}

/// How much is picked and on which model, and the way to let it go.
fn picked(ui: &mut egui::Ui, window: &mut Window) {
    let picking = window.tools.repair.picking;
    let (scene, repairs) = (&window.doc.scene, &mut window.doc.repairs);
    let finding = picking == Picking::Edges
        && scene
            .targets()
            .any(|object| repairs.found.open_edges(object).is_none());
    let empty = if picking == Picking::Edges && repairs.found.all_closed(scene) {
        tr!("repair-no-open-edges")
    } else {
        tr!("repair-nothing-picked")
    };
    let hand = &mut repairs.hand;
    let mut clear = false;
    section(ui, &tr!("repair-picked"), None, |ui| {
        if finding {
            busy(ui, &tr!("repair-finding-edges"), None, false);
            return;
        }
        let rows = match picking {
            Picking::Faces => hand.picked_faces(scene).map(|(object, count)| {
                vec![
                    (tr!("repair-picked-model"), object.name.clone()),
                    (tr!("repair-picked-faces"), count.to_string()),
                ]
            }),
            Picking::Edges => {
                let edges = hand.picked_edges(scene).len();
                (edges > 0).then(|| {
                    vec![(
                        tr!("repair-picked-edges"),
                        tr!("repair-of-two", count = edges),
                    )]
                })
            }
        };
        match rows {
            Some(rows) => {
                readings(ui, &rows);
                clear = quiet_button(ui, icon::CANCEL, &tr!("repair-clear-picks"), false, true)
                    .clicked();
            }
            None => tone(ui, &empty, theme::colors().text_low),
        }
    });
    if clear {
        hand.clear();
    }
}

/// Turning or dropping the picked faces, or bridging the two picked edges; the bar while
/// a change runs.
pub fn action(ui: &mut egui::Ui, window: &mut Window) {
    let (scene, hand) = (&window.doc.scene, &mut window.doc.repairs.hand);
    let status = &mut window.machine.status;
    if hand.is_busy() {
        busy(ui, &tr!("repair-editing"), None, false);
        return;
    }
    let asked = match window.tools.repair.picking {
        Picking::Faces => {
            let none = hand.picked_faces(scene).is_none();
            let blocked = none.then(|| tr!("repair-pick-faces-first"));
            let drop = companion_button(ui, icon::REMOVE, &tr!("repair-delete-faces"), !none);
            let flip = primary_button(ui, icon::FLIP, &tr!("repair-flip-faces"), !none);
            match (blocked_hover(flip, blocked).clicked(), drop.clicked()) {
                (true, _) => Some(HandEdit::Flip),
                (_, true) => Some(HandEdit::Remove),
                _ => None,
            }
        }
        Picking::Edges => {
            let two = hand.picked_edges(scene).len() == 2;
            let blocked = (!two).then(|| tr!("repair-pick-two-edges"));
            let bridge = primary_button(ui, icon::LINKED, &tr!("repair-bridge"), two);
            blocked_hover(bridge, blocked)
                .clicked()
                .then_some(HandEdit::Bridge)
        }
    };
    if let Some(edit) = asked {
        hand.start(scene, edit, status);
    }
}
