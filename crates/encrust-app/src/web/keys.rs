//! Keeps the window's chords from the browser, which acts only on the keys eframe names.

use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;

use crate::shortcuts::kept_from_browser;

/// Listens on the whole page from the capture phase, so a chord pressed while a field has the
/// focus is kept as well; the window still sees every key.
pub fn keep_from_browser() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let keydown = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|event| {
        if pressed(&event).is_some_and(|(key, modifiers)| kept_from_browser(key, modifiers)) {
            event.prevent_default();
        }
    });
    window.add_event_listener_with_callback_and_bool(
        "keydown",
        keydown.into_js_value().unchecked_ref(),
        true,
    )
}

/// The key and modifiers as eframe reads them, the physical key standing in on a non-Latin
/// layout.
fn pressed(event: &web_sys::KeyboardEvent) -> Option<(egui::Key, egui::Modifiers)> {
    let key = egui::Key::from_name(&event.key()).or_else(|| egui::Key::from_name(&event.code()))?;
    let modifiers = egui::Modifiers {
        alt: event.alt_key(),
        ctrl: event.ctrl_key(),
        shift: event.shift_key(),
        mac_cmd: event.meta_key(),
        command: event.ctrl_key() || event.meta_key(),
    };
    Some((key, modifiers))
}
