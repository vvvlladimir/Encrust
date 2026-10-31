use super::list::List;
use super::rows::{InHand, Offered, Presets};

/// BUG: with a hand preset in use and nothing Auto built standing, the automatic preset was
/// listed nowhere, so it could not be picked back.
#[test]
fn an_idle_automatic_preset_is_listed_among_the_profiles() {
    let presets = Presets {
        autos: vec![Offered {
            id: "auto".to_owned(),
            name: "Automatic".to_owned(),
            weight: 0.0,
        }],
        shapes: Vec::new(),
        by_hand: Vec::new(),
    };
    let hand = InHand {
        name: "By hand".to_owned(),
        edited: false,
        in_auto: false,
        matching: None,
        auto_source: None,
        source: None,
    };
    let ctx = egui::Context::default();
    crate::ui::theme::apply(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let list = List {
            rows: &[],
            presets: &presets,
            hand: &hand,
            shaping: false,
        };
        list.show(ui, &mut Vec::new());
    });
    output.textures_delta.clear();
    let painted = output.shapes.iter().any(|clipped| {
        matches!(&clipped.shape, egui::Shape::Text(text) if text.galley.text() == "Automatic")
    });
    assert!(painted, "the automatic preset is offered under Profiles");
}
