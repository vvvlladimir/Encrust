//! The render view: the plate drawn in the resin, with references of known size beside it.

use super::*;
use crate::camera::OrbitCamera;
use crate::render::vertex::ModelInstance;

fn resin() -> Color32 {
    theme::resin_swatch([0xc8, 0x28, 0x28])
}

#[test]
fn the_render_view_draws_every_model_in_the_resin() {
    let scene = a_closed_box();
    let draws = Draws::of(&scene, NOT_MARKED, None, &[], None, None, Some(resin()));
    let expected = ModelInstance::new(scene.objects()[0].transform, resin(), NOT_MARKED);
    assert_eq!(draws.models.len(), 1, "one box, one model drawn");
    assert_eq!(
        bytemuck::bytes_of(&draws.models[0].instance),
        bytemuck::bytes_of(&expected),
        "expected the box in the resin's colour"
    );
}

#[test]
fn references_stand_beside_the_plate_only_in_the_render_view() {
    let plate = BuildPlate::default();
    let markers = |print_look| {
        ViewportCallback::new(
            &Scene::default(),
            &plate,
            &OrbitCamera::framing_plate(&plate),
            ViewOptions::default(),
            Shading {
                print_look,
                ..Shading::default()
            },
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
        )
        .markers
        .len()
    };
    let look = PrintLook {
        resin: resin(),
        layer_height_mm: 0.05,
    };
    assert_eq!(
        markers(None),
        0,
        "the working view lays nothing beside the plate"
    );
    assert_eq!(
        markers(Some(look)),
        3,
        "a card, a coin and a miniature's base"
    );
}
