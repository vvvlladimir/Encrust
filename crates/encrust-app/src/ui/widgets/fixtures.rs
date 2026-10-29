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
