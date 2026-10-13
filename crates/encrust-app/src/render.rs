//! The wgpu viewport: the build plate, its grid and the shaded models, painted inside an
//! egui panel through `egui_wgpu::CallbackTrait`. See `docs/decisions/0014-viewport-rendering.md`.

mod callback;
mod frame_time;
mod gpu;
mod grid;
mod label;
mod lod;
#[cfg(test)]
mod offscreen;
mod target;
mod vertex;

pub use callback::{Banding, Shading, ViewportCallback};
pub use gpu::CutLine;
#[cfg(not(target_arch = "wasm32"))]
pub use gpu::MULTISAMPLING;
pub use grid::AXIS_LENGTH_MM;
pub use label::prime as prime_label;
pub use lod::{Budget, Candidate, Light, Lods, allot};

use gpu::ViewportResources;

/// Asks the device for the card's own clock over a pass where the adapter has one: the
/// light copies drawn while the camera moves are budgeted by it.
#[cfg(not(target_arch = "wasm32"))]
pub fn ask_for_timestamps(options: &mut egui_wgpu::WgpuConfiguration) {
    let egui_wgpu::WgpuSetup::CreateNew(create_new) = &mut options.wgpu_setup else {
        return;
    };
    let base = std::sync::Arc::clone(&create_new.device_descriptor);
    create_new.device_descriptor = std::sync::Arc::new(move |adapter| {
        let mut descriptor = base(adapter);
        descriptor.required_features |= adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        descriptor
    });
}

/// Builds the viewport's pipelines and buffers and hands them to egui, which keeps them
/// for the lifetime of the window and lends them back to every paint callback.
pub fn install(render_state: &egui_wgpu::RenderState) {
    let resources = ViewportResources::new(&render_state.device, render_state.target_format);
    #[cfg(not(target_arch = "wasm32"))]
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(resources);
    #[cfg(target_arch = "wasm32")]
    HELD.with(|held| *held.borrow_mut() = Some(resources));
}

// In a browser a GPU object belongs to the page's thread, so it cannot sit in egui's map,
// which is shared between threads; the page's thread keeps it instead. Callbacks run there.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static HELD: std::cell::RefCell<Option<ViewportResources>> =
        const { std::cell::RefCell::new(None) };
}

/// Lends the viewport's resources to a callback, from wherever this platform keeps them.
fn with_resources<R>(
    #[cfg_attr(
        target_arch = "wasm32",
        expect(unused_variables, reason = "kept beside it")
    )]
    callback_resources: &mut egui_wgpu::CallbackResources,
    lend: impl FnOnce(&mut ViewportResources) -> R,
) -> Option<R> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        callback_resources.get_mut::<ViewportResources>().map(lend)
    }
    #[cfg(target_arch = "wasm32")]
    {
        HELD.with(|held| held.borrow_mut().as_mut().map(lend))
    }
}

/// The same, to read.
fn resources<R>(
    #[cfg_attr(
        target_arch = "wasm32",
        expect(unused_variables, reason = "kept beside it")
    )]
    callback_resources: &egui_wgpu::CallbackResources,
    lend: impl FnOnce(&ViewportResources) -> R,
) -> Option<R> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        callback_resources.get::<ViewportResources>().map(lend)
    }
    #[cfg(target_arch = "wasm32")]
    {
        HELD.with(|held| held.borrow().as_ref().map(lend))
    }
}
