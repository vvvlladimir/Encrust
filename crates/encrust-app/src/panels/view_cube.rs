//! The cube in the viewport's top right corner: which way the camera faces, and a click on
//! a face, an edge or a corner to look from there. Drawn with egui shapes, so it needs no
//! pass of its own; see ADR 0219.

use core_geometry::{Mat4, Vec3};
use egui::{Align2, Id, Pos2, Rect, Sense, Shape, Stroke, Vec2, vec2};

use crate::camera::{CameraTurn, OrbitCamera};
use crate::panels::Window;
use crate::ui::{icon, icon_button, theme};

/// Points between the cube and the corner of the viewport, and the square it is drawn in.
const INSET: f32 = 12.0;
const SIZE: f32 = 76.0;

/// How much of a face's half-width its middle takes; the band past it belongs to an edge,
/// and where two bands cross, to a corner.
const FACE_MIDDLE: f32 = 0.5;

/// How long the camera takes to swing to a picked view, seconds.
const TURN_S: f64 = 0.35;

/// A face turned further from the camera than this is drawn as nothing: seen edge on it
/// would be a hairline along the cube's side.
const SEEN_FACING: f32 = 0.08;

/// How tall a face's name is and how wide it may be, in face units of the two across it.
const NAME_HEIGHT: f32 = 0.5;
const NAME_WIDTH: f32 = 1.6;

/// One face of the cube: its outward normal, which way its name reads, and which way is up
/// for the name. All three are plate axes.
struct Face {
    normal: Vec3,
    across: Vec3,
    up: Vec3,
    name: &'static str,
}

const FACES: [Face; 6] = [
    Face {
        normal: Vec3::Z,
        across: Vec3::X,
        up: Vec3::Y,
        name: "Top",
    },
    Face {
        normal: Vec3::NEG_Z,
        across: Vec3::X,
        up: Vec3::NEG_Y,
        name: "Bottom",
    },
    Face {
        normal: Vec3::NEG_Y,
        across: Vec3::X,
        up: Vec3::Z,
        name: "Front",
    },
    Face {
        normal: Vec3::Y,
        across: Vec3::NEG_X,
        up: Vec3::Z,
        name: "Back",
    },
    Face {
        normal: Vec3::X,
        across: Vec3::Y,
        up: Vec3::Z,
        name: "Right",
    },
    Face {
        normal: Vec3::NEG_X,
        across: Vec3::NEG_Y,
        up: Vec3::Z,
        name: "Left",
    },
];

/// One of the nine cells a face is cut into, on screen, and the view it stands for: the
/// face's own normal plus the normals of the edges or corner it touches.
struct Cell {
    corners: [Pos2; 4],
    towards: Vec3,
}

/// Top right of the viewport: the cube and, under it while the pointer is over the cube,
/// the way back to the opening view.
pub fn ui(ui: &egui::Ui, window: &mut Window, viewport: Rect) {
    let id = Id::new("view-cube");
    // Last frame's area, which holds the room for Home whether or not it is drawn, so the
    // button does not vanish under the pointer on its way down to it.
    let hovered = ui
        .ctx()
        .memory(|memory| memory.area_rect(id))
        .zip(ui.ctx().pointer_hover_pos())
        .is_some_and(|(area, at)| area.contains(at));
    egui::Area::new(id)
        .order(egui::Order::Middle)
        .fixed_pos(viewport.right_top() + vec2(-INSET, INSET))
        .pivot(Align2::RIGHT_TOP)
        .constrain_to(viewport)
        .show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 4.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                if let Some(towards) = cube(ui, &window.view.camera) {
                    let to = window.view.camera.seen_from(towards);
                    turn(ui, window, to);
                }
                let home = |ui: &mut egui::Ui| icon_button(ui, icon::HOME, "Home view");
                if ui.add_visible(hovered, home).clicked() {
                    let to = OrbitCamera {
                        orthographic: window.view.camera.orthographic,
                        ..OrbitCamera::framing_plate(&window.doc.plate)
                    };
                    turn(ui, window, to);
                }
            });
        });
}

/// Starts the camera swinging to `to` from wherever it stands now.
fn turn(ui: &egui::Ui, window: &mut Window, to: OrbitCamera) {
    window.view.turn = Some(CameraTurn {
        from: window.view.camera,
        to,
        started_s: ui.input(|input| input.time),
        length_s: TURN_S,
    });
    ui.ctx().request_repaint();
}

/// Draws the cube as `camera` sees it and returns the view a click on it asked for.
fn cube(ui: &mut egui::Ui, camera: &OrbitCamera) -> Option<Vec3> {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(SIZE), Sense::click());
    let view = camera.view();
    let faces: Vec<(&Face, Vec<Cell>)> = FACES
        .iter()
        .filter(|face| view.transform_vector3(face.normal).z > SEEN_FACING)
        .map(|face| (face, cells(face, &view, rect)))
        .collect();
    let hovered = response.hover_pos().and_then(|at| {
        faces
            .iter()
            .flat_map(|(_, cells)| cells)
            .find(|cell| contains(&cell.corners, at))
            .map(|cell| cell.towards)
    });

    let colors = theme::colors();
    let painter = ui.painter();
    for (face, cells) in &faces {
        for cell in cells {
            let fill = match Some(cell.towards) == hovered {
                true => colors.accent_wash,
                false => colors.raised,
            };
            painter.add(Shape::convex_polygon(
                cell.corners.to_vec(),
                fill,
                Stroke::new(0.5, colors.hairline),
            ));
        }
        let outline = [0, 2, 8, 6].map(|index| corner_of(&cells[index], index));
        painter.add(Shape::closed_line(
            outline.to_vec(),
            Stroke::new(1.0, colors.line),
        ));
        name(painter, face, &view, rect);
    }
    if hovered.is_some() {
        response
            .clone()
            .on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    response.clicked().then_some(hovered).flatten()
}

/// The corner of a face that cell `index` of its 3 by 3 grid holds, for a corner cell.
fn corner_of(cell: &Cell, index: usize) -> Pos2 {
    match index {
        0 => cell.corners[0],
        2 => cell.corners[1],
        8 => cell.corners[2],
        _ => cell.corners[3],
    }
}

/// The nine cells of `face` on screen, row by row from the bottom left of its name.
fn cells(face: &Face, view: &Mat4, rect: Rect) -> Vec<Cell> {
    let bounds = [-1.0, -FACE_MIDDLE, FACE_MIDDLE, 1.0];
    let at = |across: f32, up: f32| {
        project(
            face.normal + face.across * across + face.up * up,
            view,
            rect,
        )
    };
    let mut cells = Vec::with_capacity(9);
    for row in 0..3 {
        for column in 0..3 {
            let (left, right) = (bounds[column], bounds[column + 1]);
            let (bottom, top) = (bounds[row], bounds[row + 1]);
            cells.push(Cell {
                corners: [
                    at(left, bottom),
                    at(right, bottom),
                    at(right, top),
                    at(left, top),
                ],
                towards: face.normal
                    + face.across * (column as f32 - 1.0)
                    + face.up * (row as f32 - 1.0),
            });
        }
    }
    cells
}

/// A point of the cube, a unit from its middle to a face, onto the square it is drawn in.
/// Only the camera's turn is taken: the cube is drawn without perspective.
fn project(point: Vec3, view: &Mat4, rect: Rect) -> Pos2 {
    // The cube's corner is √3 from its middle, which is what has to fit in half the square.
    let scale = rect.width() / 2.0 / 3.0_f32.sqrt();
    let seen = view.transform_vector3(point);
    rect.center() + vec2(seen.x, -seen.y) * scale
}

/// Whether `point` is inside the convex quadrilateral `corners`, in either winding.
fn contains(corners: &[Pos2; 4], point: Pos2) -> bool {
    let sides = (0..4).map(|index| {
        let (from, to) = (corners[index], corners[(index + 1) % 4]);
        (to - from).x * (point - from).y - (to - from).y * (point - from).x
    });
    let (mut left, mut right) = (false, false);
    for side in sides {
        left |= side > 0.0;
        right |= side < 0.0;
    }
    !(left && right)
}

/// The face's name lying on it: the laid-out text is carried onto the face's plane and
/// projected with it, so it turns and foreshortens as the face does.
fn name(painter: &egui::Painter, face: &Face, view: &Mat4, rect: Rect) {
    let galley = painter.layout_no_wrap(
        face.name.to_owned(),
        theme::small(),
        theme::colors().text_mid,
    );
    let size = galley.rect.size();
    // Face units per point: the name as tall as a share of the face, and never wider.
    let scale = (NAME_HEIGHT / size.y.max(1.0)).min(NAME_WIDTH / size.x.max(1.0));
    let middle = galley.rect.center();
    // A laid-out row samples the font atlas in texels; a mesh painted on its own in
    // fractions of it.
    let [width_px, height_px] = painter.ctx().fonts(|fonts| fonts.font_image_size());
    let atlas = vec2(width_px as f32, height_px as f32);
    let mut mesh = egui::Mesh::default();
    for placed in &galley.rows {
        let row = &placed.row.visuals.mesh;
        let base = mesh.vertices.len() as u32;
        mesh.indices
            .extend(row.indices.iter().map(|index| base + index));
        mesh.vertices.extend(row.vertices.iter().map(|vertex| {
            let at = placed.pos + vertex.pos.to_vec2() - middle;
            let on_face = face.normal + face.across * at.x * scale - face.up * at.y * scale;
            egui::epaint::Vertex {
                pos: project(on_face, view, rect),
                uv: (vertex.uv.to_vec2() / atlas).to_pos2(),
                color: vertex.color,
            }
        }));
    }
    painter.add(Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE))
    }

    /// Every face, edge and corner of a cube once: six, twelve and eight.
    #[test]
    fn the_cells_of_the_faces_stand_for_all_twenty_six_views() {
        let view = OrbitCamera::default().view();
        let mut views: Vec<[i32; 3]> = FACES
            .iter()
            .flat_map(|face| cells(face, &view, square()))
            .map(|cell| cell.towards.to_array().map(|axis| axis.round() as i32))
            .collect();
        views.sort_unstable();
        views.dedup();
        assert_eq!(views.len(), 26);
        assert!(!views.contains(&[0, 0, 0]));
    }

    #[test]
    fn the_front_face_faces_a_camera_standing_in_front() {
        let view = OrbitCamera::default().seen_from(Vec3::NEG_Y).view();
        let front = &FACES[2];
        let middle = &cells(front, &view, square())[4];
        assert_eq!(middle.towards, Vec3::NEG_Y);
        assert!(
            contains(&middle.corners, square().center()),
            "seen straight on, the front face's middle is the middle of the square"
        );
    }

    #[test]
    fn a_point_outside_a_cell_is_not_in_it() {
        let corners = [
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 0.0),
            Pos2::new(10.0, 10.0),
            Pos2::new(0.0, 10.0),
        ];
        assert!(contains(&corners, Pos2::new(5.0, 5.0)));
        assert!(!contains(&corners, Pos2::new(15.0, 5.0)));
    }
}
