//! The wgpu viewport: the build plate, its grid and the shaded models, painted inside an
//! egui panel through `egui_wgpu::CallbackTrait`. See `docs/decisions/0014-viewport-rendering.md`.

mod callback;
mod gpu;
mod grid;
mod label;
mod machine;
#[cfg(test)]
mod offscreen;
mod vertex;

pub use callback::{Banding, Shading, ViewportCallback};
pub use gpu::{DEPTH_BUFFER_BITS, MULTISAMPLING, STENCIL_BUFFER_BITS};
pub use label::prime as prime_label;

use gpu::ViewportResources;

/// Builds the viewport's pipelines and buffers and hands them to egui, which keeps them
/// for the lifetime of the window and lends them back to every paint callback.
pub fn install(render_state: &egui_wgpu::RenderState) {
    let resources = ViewportResources::new(&render_state.device, render_state.target_format);
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(resources);
}
