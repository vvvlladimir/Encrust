use std::sync::Arc;

use super::ViewportCallback;
use crate::highlight::Highlight;
use crate::render::gpu::ModelDraw;
use crate::render::vertex::{ModelInstance, NOT_MARKED, WideLine};
use crate::ui::theme;

impl ViewportCallback {
    /// The same frame with `highlight` marked over the models: faces in patches of their
    /// own, edges as wide lines, each in its own colour, in the order it lists them.
    pub fn highlighting(mut self, highlight: &Highlight) -> Self {
        let size = core_geometry::Vec2::new(self.rect.width(), self.rect.height());
        for mark in &highlight.faces {
            self.models.push(ModelDraw::whole(
                Arc::clone(&mark.mesh),
                ModelInstance::new(mark.transform, mark.colour, NOT_MARKED),
            ));
        }
        for mark in &highlight.edges {
            self.lines.wide.extend(
                mark.edges
                    .iter()
                    .map(|&ends| WideLine::new(ends, mark.colour, theme::MARK_LINE_W, size)),
            );
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Transform, Vec3, cuboid};

    use super::*;
    use crate::camera::OrbitCamera;
    use crate::highlight::{EdgeMark, FaceMark};
    use crate::plate::BuildPlate;
    use crate::scene::Scene;
    use crate::workspace::ViewOptions;

    #[test]
    fn marked_faces_and_edges_are_drawn_each_in_its_own_colour() {
        let plate = BuildPlate::default();
        let frame = || {
            ViewportCallback::new(
                &Scene::default(),
                &plate,
                &OrbitCamera::framing_plate(&plate),
                ViewOptions::default(),
                super::super::Shading::default(),
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
            )
        };
        let plain = frame();
        let (rest, focus) = (theme::scene().fault_rest, theme::scene().fault);
        let highlight = Highlight {
            faces: vec![FaceMark {
                mesh: Arc::new(cuboid(Vec3::ZERO, Vec3::ONE)),
                transform: Transform::default(),
                colour: rest,
            }],
            edges: vec![
                EdgeMark {
                    edges: vec![[Vec3::ZERO, Vec3::X]],
                    colour: rest,
                },
                EdgeMark {
                    edges: vec![[Vec3::Y, Vec3::Z]],
                    colour: focus,
                },
            ],
        };
        let marked = frame().highlighting(&highlight);

        assert_eq!(marked.models.len(), plain.models.len() + 1);
        let size = core_geometry::Vec2::splat(100.0);
        let ends = [
            WideLine::new([Vec3::ZERO, Vec3::X], rest, theme::MARK_LINE_W, size),
            WideLine::new([Vec3::Y, Vec3::Z], focus, theme::MARK_LINE_W, size),
        ];
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&marked.lines.wide),
            bytemuck::cast_slice::<_, u8>(&ends),
            "each edge runs end to end in the colour of its mark, in order"
        );
    }
}
