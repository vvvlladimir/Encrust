//! What the window needs from a browser that a desktop gives it for free: threads, files,
//! and somewhere to keep its settings. Built for wasm32 alone; see ADR 0181 and
//! `docs/design/web-build.md`.

pub mod files;
pub mod opfs;
pub mod profiles;
pub mod thread;

use std::cell::RefCell;

thread_local! {
    /// The window's context, so that something the browser finishes between frames can
    /// ask for the next one.
    static CONTEXT: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
}

/// Lets the browser wake the window.
pub fn hold_context(ctx: &egui::Context) {
    CONTEXT.with(|context| *context.borrow_mut() = Some(ctx.clone()));
}

pub fn repaint() {
    CONTEXT.with(|context| {
        if let Some(ctx) = context.borrow().as_ref() {
            ctx.request_repaint();
        }
    });
}

/// What the page remembers under `key`, which outlives the tab.
pub fn remembered(key: &str) -> Option<String> {
    local_storage()?.get_item(key).ok().flatten()
}

/// Remembers `text` under `key`. A browser that will not keep it loses nothing the user
/// asked for, so a failure is silent.
pub fn remember(key: &str, text: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, text);
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
