use wasm_bindgen::prelude::*;

/// Opens the window on `canvas`. `bindings` is the URL of this module's JavaScript, which
/// every worker the window starts imports to run on the same memory.
#[wasm_bindgen]
pub async fn start(canvas: web_sys::HtmlCanvasElement, bindings: String) -> Result<(), JsValue> {
    encrust_app::run_web(canvas, bindings).await
}
