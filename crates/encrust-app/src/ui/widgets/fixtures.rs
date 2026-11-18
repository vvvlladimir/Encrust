//! What a widget's test draws it in: a context with the window's fonts and tokens.

use egui::{Context, RawInput, Ui};

use crate::ui::theme;

/// A context dressed as the window is.
pub fn context() -> Context {
    let ctx = Context::default();
    theme::apply(&ctx);
    ctx
}

/// One frame of `ctx` as `input` drives it, `add` drawing in its root.
pub fn frame(ctx: &Context, input: RawInput, add: impl FnMut(&mut Ui)) {
    let mut add = add;
    let mut output = ctx.run_ui(input, |ui| add(ui));
    // No renderer takes the font atlas here.
    output.textures_delta.clear();
}

/// Whether a frame drawing `add` paints egui's warning of an id used twice.
pub(in crate::ui::widgets) fn clashes(add: impl FnMut(&mut Ui)) -> bool {
    let ctx = context();
    ctx.options_mut(|options| options.warn_on_id_clash = true);
    let mut add = add;
    let mut said = false;
    for _ in 0..2 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| add(ui));
        // No renderer takes the font atlas here.
        output.textures_delta.clear();
        said = output.shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => text.galley.text().contains("use of"),
            _ => false,
        });
    }
    said
}
